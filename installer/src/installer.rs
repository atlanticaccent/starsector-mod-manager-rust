use std::{
  fs::{copy, create_dir_all},
  future::Future,
  path::{Path, PathBuf},
  sync::Arc,
};

use remove_dir_all::remove_dir_all;
use tempfile::{tempdir, TempDir};
use tokio::{fs::rename, io::AsyncWriteExt, time::timeout};

mod hybrid_path;
mod search;

pub use hybrid_path::HybridPath;

use crate::{
  installer::search::ModSearch,
  traits::{EntryExt, EntryUpdate},
  Entry, InstallError, InstallerDelegate,
};

pub enum Request<T, U = ()> {
  Initial(Vec<HybridPath>, PathBuf),
  Resumed(Box<T>, HybridPath, PathBuf),
  Download { remote_data: U, old_path: PathBuf },
}

pub struct EnrichedEntry<T: Entry>(pub T);

type InstallerError<T> = InstallError<<T as InstallerDelegate>::Entry>;

pub trait InstallerExt: InstallerDelegate
where
  Self: 'static,
{
  fn install(
    &self,
    request: Request<Self::Entry, Self::EntryUpdate>,
  ) -> impl Future<Output = ()> + Send {
    fn install_impl<'a, INST: InstallerExt>(
      installer: &'a INST,
      request: Request<INST::Entry, INST::EntryUpdate>,
    ) -> impl Future<Output = Result<(), InstallerError<INST>>> + Send + 'a {
      async move {
        match request {
          Request::Initial(mut targets, parent_dest) if targets.len() == 1 => {
            installer
              .handle_path(targets.pop().unwrap(), parent_dest)
              .await
          }
          Request::Initial(targets, parent_dest) => {
            let _count = targets.len();
            let parent_dest: Arc<Path> = parent_dest.into();

            let futures = targets
              .into_iter()
              .map(|target| installer.handle_path(target, parent_dest.clone()));

            futures_util::future::try_join_all(futures).await?;

            Ok(())
          }
          Request::Resumed(entry, path, existing) => {
            installer.handle_delete(*entry, path, existing).await
          }
          Request::Download {
            remote_data,
            old_path,
          } => installer.handle_auto(remote_data, old_path).await,
        }
      }
    }

    async move {
      let res = install_impl(self, request).await;

      if let Err(err) = res {
        self.error_handler(err);
      }
    }
  }

  fn handle_path<'a>(
    &'a self,
    source: HybridPath,
    parent_dest: impl AsRef<Path> + Send + 'static,
  ) -> impl Future<Output = Result<(), InstallerError<Self>>> + Send + 'a {
    async move {
      let parent_dest = parent_dest.as_ref();
      let path = source.get_path_copy();
      let file_name = path.file_name().map_or_else(
        || String::from("unknown"),
        |f| f.to_string_lossy().to_string(),
      );

      let mod_folder = if path.is_file() {
        let decompress = tokio::task::spawn_blocking(move || Self::decompress(&path)).await??;
        drop(source);
        HybridPath::Temp(Arc::new(decompress), file_name.clone(), None)
      } else {
        source
      };

      let dir = mod_folder.get_path_copy();

      let entry_paths = match &mod_folder {
        HybridPath::PathBuf(_) | HybridPath::Temp(_, _, None) => {
          timeout(
            std::time::Duration::from_millis(500),
            tokio::task::spawn_blocking(move || ModSearch::new(dir).exhaustive()),
          )
          .await??
        }
        HybridPath::Temp(_, _, Some(path)) => Ok(vec![path.clone()]),
      }
      .map_err(InstallError::ModSearchError)?;

      #[allow(irrefutable_let_patterns)]
      if entry_paths.len() > 1 {
        let found = futures_util::future::try_join_all(
          entry_paths
            .into_iter()
            .map(|path| Self::Entry::parse_ext(path)),
        )
        .await?;

        self.multiple_handler(mod_folder, found);
      } else if let Some(path) = entry_paths.first() {
        let mut entry = Self::Entry::parse_ext(path).await?;
        if self.check_conflict(&entry).await {
          // note: this is probably the way wrong way of doing this
          // instead, just submit the new entry if it doesn't conflict with an existing
          // path, _then_ detect the conflict that way there's less chance an
          // existing ID gets missed due to the ID list effectively getting cached when
          // this function starts

          self.overwrite_handler(entry.id().into(), mod_folder.with_path(path), entry)
        } else if let target_path = entry.destination_folder(&parent_dest)
          && target_path.exists()
          && target_path.is_dir()
          && !location_is_empty(&target_path)
        {
          self.overwrite_handler(target_path.into(), mod_folder.with_path(path), entry)
        } else {
          let destination = entry.destination_folder(&parent_dest);

          move_or_copy(path.clone(), destination.clone()).await;

          entry
            .enrich(self.context(), destination)
            .await
            .map_err(InstallError::EntryEnrichmentError)?;
          self.completed_handler(EnrichedEntry(entry));
        }
      } else {
        Err(InstallError::ModSearchErrorUnknown)?;
      }

      Ok(())
    }
  }

  fn handle_delete(
    &self,
    mut entry: Self::Entry,
    new_path: HybridPath,
    old_path: PathBuf,
  ) -> impl Future<Output = Result<(), InstallerError<Self>>> + Send {
    async move {
      let origin = new_path.get_path_copy();

      if origin != old_path && old_path.exists() {
        remove_dir_all(&old_path)?;
      }

      move_or_copy(origin, old_path.clone()).await;
      entry
        .enrich(self.context(), old_path)
        .await
        .map_err(InstallError::EntryEnrichmentError)?;

      self.completed_handler(EnrichedEntry(entry));

      Ok(())
    }
  }

  fn handle_auto(
    &self,
    remote_data: Self::EntryUpdate,
    old_path: PathBuf,
  ) -> impl Future<Output = Result<(), InstallerError<Self>>> + Send {
    async move {
      let url = remote_data.url();
      let file = self.download(&url).await?;
      let path = file.path().to_path_buf();
      let temp = tokio::task::spawn_blocking(move || Self::decompress(&path)).await??;

      let path = temp.path().to_owned();
      let source = url.clone();

      let path = timeout(
        std::time::Duration::from_millis(500),
        tokio::task::spawn_blocking(move || ModSearch::new(path).first()),
      )
      .await??
      .map_err(InstallError::ModSearchError)?
      .ok_or(InstallError::ModSearchErrorUnknown)?;

      let entry = Self::Entry::parse_ext(&path).await?;

      let hybrid = HybridPath::Temp(Arc::new(temp), source, Some(path));
      if remote_data.matches(&entry) {
        return self.handle_delete(entry, hybrid, old_path).await;
      }

      Ok(())
    }
  }

  fn decompress(path: &Path) -> Result<TempDir, InstallerError<Self>> {
    let source = std::fs::File::open(path)?;

    let temp_dir = tempdir()?;
    let mime_type = infer::get_from_path(path)?
      .ok_or(InstallError::Mime)?
      .mime_type();

    match mime_type {
      "application/vnd.rar" | "application/x-rar-compressed" => {
        unrar::Archive::new(path.to_string_lossy().to_string())
          .extract_to(temp_dir.path().to_string_lossy().to_string())
          .map_err(|e| InstallError::Unrar(e.to_string()))?
          .process()
          .map_err(|e| InstallError::Unrar(e.to_string()))?;
      }
      _ => compress_tools::uncompress_archive(
        source,
        temp_dir.path(),
        compress_tools::Ownership::Ignore,
      )?,
    }

    Ok(temp_dir)
  }

  fn download(
    &self,
    url: &str,
  ) -> impl Future<Output = Result<tempfile::NamedTempFile, InstallerError<Self>>> + Send {
    self.download_in(url, Option::<&Path>::None)
  }

  fn download_in(
    &self,
    url: &str,
    path: Option<impl AsRef<Path> + Send>,
  ) -> impl Future<Output = Result<tempfile::NamedTempFile, InstallerError<Self>>> + Send {
    static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

    async move {
      let file = if let Some(path) = path {
        tempfile::NamedTempFile::new_in(path)
      } else {
        tempfile::NamedTempFile::new()
      }?;
      let client = reqwest::ClientBuilder::default()
        .redirect(reqwest::redirect::Policy::limited(200))
        .user_agent(APP_USER_AGENT)
        .build()?;

      let mut res = client.get(url).send().await?;

      // let name = res
      //   .headers()
      //   .get(reqwest::header::CONTENT_DISPOSITION)
      //   .and_then(|v| v.to_str().ok())
      //   .and_then(|v| v.rsplit_once("filename="))
      //   .map_or_else(
      //     || {
      //       Url::parse(&url)
      //         .ok()
      //         .and_then(|url| {
      //           url
      //             .path_segments()
      //             .and_then(std::iter::Iterator::last)
      //             .map(std::string::ToString::to_string)
      //         })
      //         .unwrap_or(url)
      //     },
      //     |(_, filename)| filename.to_string(),
      //   );

      // let total = res.content_length();
      // let mut current_total = 0.0;
      let (handle, temp_path) = file.into_parts();
      let mut file_adapter = tokio::fs::File::from_std(handle);
      while let Some(chunk) = res.chunk().await? {
        file_adapter.write_all(&chunk).await?;
        // if let Some(total) = total {
        //   current_total += chunk.len() as f64;
        //   let _ = tx.send((start, name.clone(), (current_total / total as
        // f64))); }
      }

      Ok(tempfile::NamedTempFile::from_parts(
        file_adapter.into_std().await,
        temp_path,
      ))
    }
  }
}

impl<T: InstallerDelegate + 'static> InstallerExt for T {}

fn location_is_empty(loc: &Path) -> bool {
  loc.read_dir().into_iter().flatten().flatten().count() == 0
}

async fn move_or_copy(from: PathBuf, to: PathBuf) {
  // let mount_from = find_mountpoint(&from).expect("Find origin mount point");
  // let mount_to = find_mountpoint(&to).expect("Find destination mount point");

  if rename(from.clone(), to.clone()).await.is_err() {
    tokio::task::spawn_blocking(move || copy_dir_recursive(&to, &from))
      .await
      .expect("Run blocking dir copy")
      .expect("Copy dir to new destination");
  }
}

fn copy_dir_recursive(to: &Path, from: &Path) -> std::io::Result<()> {
  if !to.exists() {
    create_dir_all(to)?;
  }

  for entry in from.read_dir()? {
    let entry = entry?;
    if entry.file_type()?.is_dir() {
      copy_dir_recursive(&to.join(entry.file_name()), &entry.path())?;
    } else if entry.file_type()?.is_file() {
      copy(entry.path(), to.join(entry.file_name()))?;
    }
  }

  Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringOrPath {
  String(String),
  Path(PathBuf),
}

impl From<String> for StringOrPath {
  fn from(string: String) -> Self {
    StringOrPath::String(string)
  }
}

impl From<PathBuf> for StringOrPath {
  fn from(path: PathBuf) -> Self {
    StringOrPath::Path(path)
  }
}

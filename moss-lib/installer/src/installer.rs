use std::{
  borrow::Cow,
  collections::VecDeque,
  error::Error,
  fs::{copy, create_dir_all},
  future::Future,
  iter::FusedIterator,
  path::{Path, PathBuf},
  sync::Arc,
};

use futures_util::{FutureExt, TryFutureExt};
use itertools::Itertools;
use tempfile::{tempdir, TempDir};
use tokio::{
  fs::rename,
  task::{self, JoinSet},
  time::timeout,
};

use crate::{Entry, InstallerDelegate};

#[derive(Clone)]
pub enum Request<T, U = ()> {
  Initial(Vec<HybridPath>, PathBuf),
  Resumed(Box<T>, HybridPath, PathBuf),
  Download {
    id: String,
    url: String,
    expected_version: U,
    old_path: PathBuf,
  },
}

pub trait InstallerExt: InstallerDelegate
where
  for<'a> <<Self as InstallerDelegate>::Entry as TryFrom<&'a Path>>::Error: Send,
  InstallError<<Self::Entry as Entry>::Error>: From<Self::InstallError>,
  InstallError<<Self::Entry as Entry>::Error>: Into<Self::InstallError>,
{
  type InstallError: Send;

  fn install<U: Send>(
    &self,
    request: Request<Self::Entry, U>,
  ) -> impl Future<Output = Result<(), Self::InstallError>> + Send {
    let installer = self.clone();
    async move {
      match request {
        Request::Initial(mut targets, parent_dest) if targets.len() == 1 => {
          installer
            .handle_path(targets.pop().unwrap(), parent_dest)
            .await
        }
        Request::Initial(targets, parent_dest) => {
          let mut handles = JoinSet::new();
          let parent_dest: Arc<Path> = parent_dest.into();
          for target in targets {
            handles.spawn(installer.clone().handle_path(target, parent_dest.clone()));
          }

          let mut sum_result = Vec::new();
          while let Some(res) = handles.join_next().await {
            let res = res
              .map_err(|err| Self::InstallError::from(InstallError::Join(err).into()))
              .flatten();

            sum_result.push(res);
          }

          let (completed, errors): (Vec<_>, Vec<_>) = sum_result.into_iter().partition_result();

          Ok(todo!())
        }
        Request::Resumed(data, hybrid_path, path_buf) => todo!(),
        Request::Download {
          id,
          url,
          expected_version,
          old_path,
        } => todo!(),
      }
    }
  }

  fn handle_path(
    self,
    source: HybridPath,
    parent_dest: impl AsRef<Path> + Send,
  ) -> impl Future<Output = Result<(), Self::InstallError>> + Send {
    async move {
      let parent_dest = parent_dest.as_ref();
      let path = source.get_path_copy();
      let file_name = path.file_name().map_or_else(
        || String::from("unknown"),
        |f| f.to_string_lossy().to_string(),
      );

      let mod_folder = if path.is_file() {
        let decompress = task::spawn_blocking(move || decompress(&path)).await??;
        HybridPath::Temp(Arc::new(decompress), file_name.clone(), None)
      } else {
        source
      };

      let dir = mod_folder.get_path_copy();

      let mod_paths = match &mod_folder {
        HybridPath::PathBuf(_) | HybridPath::Temp(_, _, None) => {
          timeout(
            std::time::Duration::from_millis(500),
            task::spawn_blocking(move || ModSearch::new(dir).exhaustive()),
          )
          .await??
        }
        HybridPath::Temp(_, _, Some(path)) => Ok(vec![path.clone()]),
      }
      .map_err(InstallError::ModSearchError)?;

      #[allow(irrefutable_let_patterns)]
      if mod_paths.len() > 1 {
        let found = mod_paths
          .into_iter()
          .filter_map(|path| Self::Entry::try_from(&path).ok())
          .collect_vec();

        self.multiple_handler(mod_folder, found);

        Ok(())
      } else if let Some(mod_path) = mod_paths.first()
            // && let mod_metadata = ModMetadata::new()
            // && mod_metadata.save(mod_path).await.is_ok()
            && let Ok(mut mod_info) = Self::Entry::try_from(mod_path)
      {
        if self.check_conflict(&mod_info).await {
          // note: this is probably the way wrong way of doing this
          // instead, just submit the new entry if it doesn't conflict with an existing
          // path, _then_ detect the conflict that way there's less chance an
          // existing ID gets missed due to the ID list effectively getting cached when
          // this function starts

          self.overwrite_handler(
            mod_info.id().into(),
            mod_folder.with_path(mod_path),
            mod_info,
          )
        } else if let target_path = mod_info.destination_folder(&parent_dest)
          && target_path.exists()
          && target_path.is_dir()
          && !location_is_empty(&target_path)
        {
          self.overwrite_handler(target_path.into(), mod_folder.with_path(mod_path), mod_info)
        } else {
          let destination = mod_info.destination_folder(&parent_dest);

          move_or_copy(mod_path.clone(), destination.clone()).await;

          mod_info
            .enrich(destination)
            .await
            .map_err(InstallError::EntryEnrichmentError)?;
          self.completed_handler(mod_info)
        }

        Ok(())
      } else {
        Err(InstallError::ModSearchErrorUnknown)
      }
    }
    .map_err(Into::into)
  }
}

impl<T: InstallerDelegate> InstallerExt for T
where
  for<'a> <<Self as InstallerDelegate>::Entry as TryFrom<&'a Path>>::Error: Send,
{
  type InstallError = InstallError<<Self::Entry as Entry>::Error>;
}

fn location_is_empty(loc: &Path) -> bool {
  loc.read_dir().into_iter().flatten().flatten().count() == 0
}

pub fn decompress<E: Error + Send>(path: &Path) -> Result<TempDir, InstallError<E>> {
  let source = std::fs::File::open(path)?;
  let temp_dir = tempdir()?;
  let mime_type = infer::get_from_path(path)?
    .ok_or(InstallError::<E>::Mime)?
    .mime_type();

  match mime_type {
    "application/vnd.rar" | "application/x-rar-compressed" => {
      #[cfg(not(target_env = "musl"))]
      unrar::Archive::new(path.to_string_lossy().to_string())
        .extract_to(temp_dir.path().to_string_lossy().to_string())
        .map_err(|e| InstallError::Unrar(e.to_string()))?
        .process()
        .map_err(|e| InstallError::Unrar(e.to_string()))?;
      // trust me I tried to de-dupe this and it's buggered
      #[cfg(target_env = "musl")]
      compress_tools::uncompress_archive(source, temp_dir.path(), compress_tools::Ownership::Ignore)
        .context(CompressTools {})?
    }
    _ => compress_tools::uncompress_archive(
      source,
      temp_dir.path(),
      compress_tools::Ownership::Ignore,
    )?,
  }

  Ok(temp_dir)
}

struct ModSearch {
  paths: VecDeque<PathBuf>,
}

impl Iterator for ModSearch {
  type Item = std::io::Result<PathBuf>;

  fn next(&mut self) -> Option<Self::Item> {
    while let Some(path) = self.paths.pop_front() {
      let folders = path.read_dir().map(|iter| {
        iter.filter_map(|entry| {
          entry
            .ok()
            .filter(|entry| {
              entry
                .file_type()
                .as_ref()
                .is_ok_and(std::fs::FileType::is_dir)
            })
            .map(|entry| entry.path())
        })
      });

      let mut res = None;

      match folders {
        Ok(folders) => self.paths.extend(folders),
        Err(err) => res = Some(Err(err)),
      }

      if path.join("mod_info.json").is_file() {
        res = Some(Ok(path));
      }

      if res.is_some() {
        return res;
      }
    }

    None
  }
}

#[allow(dead_code)]
impl ModSearch {
  pub fn new(path: impl AsRef<Path>) -> Self {
    let mut paths = VecDeque::new();
    paths.push_front(path.as_ref().to_path_buf());

    ModSearch { paths }
  }

  pub fn first(&mut self) -> std::io::Result<Option<PathBuf>> {
    self.next().transpose()
  }

  pub fn exhaustive(&mut self) -> std::io::Result<Vec<PathBuf>> {
    self.collect()
  }
}

impl FusedIterator for ModSearch {}

async fn move_or_copy(from: PathBuf, to: PathBuf) {
  // let mount_from = find_mountpoint(&from).expect("Find origin mount point");
  // let mount_to = find_mountpoint(&to).expect("Find destination mount point");

  if rename(from.clone(), to.clone()).await.is_err() {
    task::spawn_blocking(move || copy_dir_recursive(&to, &from))
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

#[derive(Debug, Clone)]
pub enum HybridPath {
  PathBuf(PathBuf),
  Temp(Arc<TempDir>, String, Option<PathBuf>),
}

impl HybridPath {
  pub fn get_path_copy(&self) -> PathBuf {
    match self {
      HybridPath::PathBuf(ref path) | HybridPath::Temp(_, _, Some(ref path)) => path.clone(),
      HybridPath::Temp(ref arc, _, None) => arc.path().to_path_buf(),
    }
  }

  pub fn with_path(mut self, path: &Path) -> Self {
    match &mut self {
      HybridPath::PathBuf(inner) => path.clone_into(inner),
      HybridPath::Temp(_, _, path_opt) => {
        path_opt.replace(path.to_owned());
      }
    };

    self
  }

  pub fn source(&self) -> Cow<str> {
    match self {
      HybridPath::PathBuf(path) => path.to_string_lossy(),
      HybridPath::Temp(_, source, _) => source.into(),
    }
  }
}

impl PartialEq for HybridPath {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::PathBuf(l0), Self::PathBuf(r0)) => l0 == r0,
      (Self::Temp(l0, l1, l2), Self::Temp(r0, r1, r2)) => {
        l0.path() == r0.path() && l1 == r1 && l2 == r2
      }
      _ => false,
    }
  }
}

impl From<PathBuf> for HybridPath {
  fn from(value: PathBuf) -> Self {
    Self::PathBuf(value)
  }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError<E: Error> {
  #[error("Failed to find mod")]
  ModSearchError(#[source] std::io::Error),
  #[error("Unknown failure to find mod")]
  ModSearchErrorUnknown,
  #[error("Entry enrichment error")]
  EntryEnrichmentError(#[source] E),
  #[error("I/O error: {0:?}")]
  Io(#[from] std::io::Error),
  #[error("Failed to determine file type")]
  Mime,
  #[error("Libarchive error: {0:?}")]
  CompressTools(#[from] compress_tools::Error),
  #[error("Error in Unrar rar decompression lib: {0:?}")]
  Unrar(String),
  #[error("Generic network error: {0:?}")]
  Network(#[from] reqwest::Error),
  #[error("Task timed out")]
  Timeout(#[from] tokio::time::error::Elapsed),
  #[error("Failed to join task/thread: {0:?}")]
  Join(#[from] tokio::task::JoinError),
  #[error("{0}")]
  Generic(#[from] anyhow::Error),
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

#[cfg(test)]
mod test {
  use std::{collections::HashSet, fs, ops::Deref, path::Path};

  use self_update::TempDir;
  use tempfile::tempdir;

  use super::ModSearch;

  fn fill_folder_with_n_mods<const N: usize>(path: impl Deref<Target = Path>) {
    for i in 0..N {
      fs::create_dir(path.join(format!("{i}"))).expect("Create fake mod dir");
      fs::File::create(path.join(format!("{i}")).join("mod_info.json"))
        .expect("Create fake mod_info.json");
    }
  }

  fn create_folder_with_n_mods<const N: usize>() -> TempDir {
    let temp_dir = tempdir().expect("Create temp dir");

    fill_folder_with_n_mods::<N>(temp_dir.path());

    temp_dir
  }

  #[test]
  fn find_first_valid_mod() {
    let mods_dir = create_folder_with_n_mods::<1>();

    let mut iter = ModSearch::new(mods_dir.path());

    assert_eq!(
      iter
        .next()
        .transpose()
        .ok()
        .flatten()
        .expect("Find first mod"),
      mods_dir.path().join("0")
    );

    assert!(iter.next().is_none());
  }

  #[test]
  fn find_all_mods() {
    let mods_dir = create_folder_with_n_mods::<5>();

    let mut iter = ModSearch::new(mods_dir.path());

    let mut path_set = HashSet::new();

    for i in 0..5 {
      let mod_path = iter
        .next()
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_else(|| panic!("Failed to find mod {}", i));

      assert!(mod_path.starts_with(mods_dir.path()));

      path_set.insert(mod_path);
    }

    assert!(iter.next().is_none());
    assert_eq!(path_set.len(), 5);
  }

  #[test]
  fn find_all_nested_mods() {
    let mods_dir = TempDir::new().unwrap();

    let nested = mods_dir.path().join(format!("nested_{}", 0));
    fs::create_dir(&nested).unwrap();
    fill_folder_with_n_mods::<2>(nested);
    let nested = mods_dir.path().join(format!("nested_{}", 1));
    fs::create_dir(&nested).unwrap();
    fill_folder_with_n_mods::<2>(nested);

    let mut iter = ModSearch::new(mods_dir.path());

    let mut path_set = HashSet::new();

    for i in 0..4 {
      let mod_path = iter
        .next()
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_else(|| panic!("Failed to find mod {}", i));

      assert!(mod_path.starts_with(mods_dir.path()));

      path_set.insert(mod_path);
    }

    assert!(iter.next().is_none());
    assert_eq!(path_set.len(), 4);
  }
}

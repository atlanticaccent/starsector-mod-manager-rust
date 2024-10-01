use std::{
  borrow::Cow,
  collections::{HashMap, VecDeque},
  fs::{copy, create_dir_all},
  io::{self, Write},
  iter::FusedIterator,
  path::{Path, PathBuf},
  sync::Arc,
};

use anyhow::bail;
use chrono::Local;
use druid::{Data, ExtEventSink, Selector, SingleUse, Target};
use itertools::Itertools;
use remove_dir_all::remove_dir_all;
use reqwest::Url;
use tempfile::{tempdir, TempDir};
use tokio::{
  fs::rename,
  task::{self, JoinSet},
  time::timeout,
};
use webview_shared::ExtEventSinkExt;

use super::{
  mod_entry::{ModMetadata, ModVersionMeta, UpdateStatus},
  overlays::Popup,
  util::{get_master_version, Tap, WebClient},
};
use crate::app::{mod_entry::ModEntry, util::LoadBalancer};

#[derive(Clone)]
pub enum Payload {
  Initial(Vec<HybridPath>),
  Resumed(Box<ModEntry>, HybridPath, PathBuf),
  Download {
    mod_id: String,
    remote_version: ModVersionMeta,
    old_path: PathBuf,
  },
}

pub const INSTALL: Selector<ChannelMessage> = Selector::new("install.message");
pub const DOWNLOAD_STARTED: Selector<(i64, String)> = Selector::new("install.download.started");
pub const DOWNLOAD_PROGRESS: Selector<Vec<(i64, String, f64)>> =
  Selector::new("install.download.progress");
pub const INSTALL_FOUND_MULTIPLE: Selector<SingleUse<(Vec<PathBuf>, HybridPath)>> =
  Selector::new("install.found_multiple.install_all");

impl Payload {
  pub async fn install(self, ext_ctx: ExtEventSink, install_dir: PathBuf, installed: Vec<String>) {
    let mods_dir = install_dir.join("mods");
    let mut handles = JoinSet::new();
    match self {
      Payload::Initial(targets) => {
        let mods_dir = Arc::new(mods_dir);
        let installed = Arc::new(installed);
        for target in targets {
          handles.spawn(handle_path(
            ext_ctx.clone(),
            target,
            mods_dir.clone(),
            installed.clone(),
          ));
        }
      }
      Payload::Resumed(entry, path, existing) => {
        handles.spawn(async move { handle_delete(ext_ctx.clone(), *entry, path, existing).await });
      }
      Payload::Download {
        mod_id,
        remote_version,
        old_path,
      } => {
        handles.spawn(handle_auto(ext_ctx, mod_id, remote_version, old_path));
      }
    }
    while handles.join_next().await.is_some() {}
  }
}

async fn handle_path(
  ext_ctx: ExtEventSink,
  source: HybridPath,
  mods_dir: Arc<PathBuf>,
  installed: Arc<Vec<String>>,
) -> anyhow::Result<()> {
  let path = source.get_path_copy();
  let file_name = path.file_name().map_or_else(
    || String::from("unknown"),
    |f| f.to_string_lossy().to_string(),
  );

  let mod_folder = if path.is_file() {
    let decompress = task::spawn_blocking(move || decompress(&path))
      .await
      .expect("Run decompression");
    match decompress {
      Ok(temp) => HybridPath::Temp(Arc::new(temp), file_name.clone(), None),
      Err(err) => {
        println!("{err:?}");
        ext_ctx
          .submit_command(
            INSTALL,
            ChannelMessage::Error(file_name, err.to_string()),
            Target::Auto,
          )
          .expect("Send error over async channel");

        return Err(err);
      }
    }
  } else {
    source
  };

  let dir = mod_folder.get_path_copy();

  let res = match &mod_folder {
    HybridPath::PathBuf(_) | HybridPath::Temp(_, _, None) => {
      timeout(
        std::time::Duration::from_millis(500),
        task::spawn_blocking(move || ModSearch::new(dir).exhaustive()),
      )
      .await??
    }
    HybridPath::Temp(_, _, Some(path)) => Ok(vec![path.clone()]),
  };

  match res {
    Ok(mod_paths) => {
      if mod_paths.len() > 1 {
        let found = mod_paths
          .into_iter()
          .filter_map(|path| ModEntry::from_file(&path, ModMetadata::default()).ok())
          .collect_vec();
        let _ = ext_ctx.submit_command_global(
          Popup::OPEN_POPUP,
          Popup::found_multiple(mod_folder.clone(), found),
        );

        Ok(())
      } else if let Some(mod_path) = mod_paths.first()
        && let mod_metadata = ModMetadata::new()
        && mod_metadata.save(mod_path).await.is_ok()
        && let Ok(mut mod_info) = ModEntry::from_file(mod_path, mod_metadata)
      {
        if let Some(id) = installed.iter().find(|existing| **existing == mod_info.id) {
          // note: this is probably the way wrong way of doing this
          // instead, just submit the new entry if it doesn't conflict with an existing
          // path, _then_ detect the conflict that way there's less chance an
          // existing ID gets missed due to the ID list effectively getting cached when
          // this function starts
          ext_ctx
            .submit_command_global(
              Popup::QUEUE_POPUP,
              Popup::overwrite(id.clone().into(), mod_folder.with_path(mod_path), mod_info),
            )
            .expect("Send query over async channel");
        } else if let Some(target_path) = mods_dir.join(&mod_info.id).pipe(|p| {
          let contents = p.read_dir().into_iter().flatten().flatten();
          (p.exists() && p.is_dir() && contents.count() > 0).then_some(p)
        }) {
          if target_path.exists() {
            let _ = remove_dir_all(&target_path);
          }

          ext_ctx
            .submit_command_global(
              Popup::QUEUE_POPUP,
              Popup::overwrite(target_path.into(), mod_folder.with_path(mod_path), mod_info),
            )
            .expect("Send query over async channel");
        } else {
          move_or_copy(mod_path.clone(), mods_dir.join(&mod_info.id)).await;

          mod_info.set_path(mods_dir.join(&mod_info.id));
          if let Some(version_checker) = mod_info.version_checker.clone() {
            let client = WebClient::new();
            mod_info.remote_version = get_master_version(
              &client,
              None,
              version_checker.remote_url.clone(),
              version_checker.id.clone(),
            )
            .await;
            mod_info.update_status = Some(UpdateStatus::from((
              &version_checker,
              &mod_info.remote_version,
            )));
          }
          ext_ctx
            .submit_command(INSTALL, ChannelMessage::Success(mod_info), Target::Auto)
            .expect("Send success over async channel");
        }

        Ok(())
      } else {
        ext_ctx
          .submit_command(
            INSTALL,
            ChannelMessage::Error(
              file_name,
              "Could not find mod folder or parse mod_info file.".to_string(),
            ),
            Target::Auto,
          )
          .expect("Send error over async channel");

        bail!("Could not find mod folder or parse mod_info.json")
      }
    }
    Err(err) => {
      ext_ctx
        .submit_command(
          INSTALL,
          ChannelMessage::Error(file_name, format!("Failed to find mod, err: {err}")),
          Target::Auto,
        )
        .expect("Send error over async channel");

      Err(err.into())
    }
  }
}

pub fn decompress(path: &Path) -> anyhow::Result<TempDir> {
  let source = std::fs::File::open(path)?;
  let temp_dir = tempdir()?;
  let mime_type = infer::get_from_path(path)?
    .ok_or(InstallError::Mime)?
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

fn copy_dir_recursive(to: &Path, from: &Path) -> io::Result<()> {
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

async fn handle_delete(
  ext_ctx: ExtEventSink,
  mut entry: ModEntry,
  new_path: HybridPath,
  old_path: PathBuf,
) -> anyhow::Result<()> {
  let origin = new_path.get_path_copy();

  if origin != old_path && old_path.exists() {
    remove_dir_all(&old_path)?;
  }

  move_or_copy(origin, old_path.clone()).await;
  entry.set_path(old_path);
  if let Some(version_checker) = entry.version_checker.clone() {
    let client = WebClient::new();
    entry.remote_version = get_master_version(
      &client,
      None,
      version_checker.remote_url.clone(),
      version_checker.id.clone(),
    )
    .await;
    entry.update_status = Some(UpdateStatus::from((
      &version_checker,
      &entry.remote_version,
    )));
  }

  ext_ctx
    .submit_command(INSTALL, ChannelMessage::Success(entry), Target::Auto)
    .expect("Send success over async channel");

  Ok(())
}

async fn handle_auto(
  ext_ctx: ExtEventSink,
  mod_id: String,
  remote_version: ModVersionMeta,
  old_path: PathBuf,
) -> anyhow::Result<()> {
  let url = remote_version.direct_download_url.as_ref().unwrap();
  let target_version = &remote_version.version;
  match download(url.clone(), ext_ctx.clone()).await {
    Ok(file) => {
      let path = file.path().to_path_buf();
      let decompress = task::spawn_blocking(move || decompress(&path))
        .await
        .expect("Run decompression");
      match decompress {
        Ok(temp) => {
          let temp = Arc::new(temp);
          let path = temp.path().to_owned();
          let source = url.clone();
          let mod_metadata = ModMetadata::new();
          if let Ok(Some(path)) = task::spawn_blocking(move || ModSearch::new(path).first()).await?
            && mod_metadata.save(&path).await.is_ok()
            && let Ok(mod_info) = ModEntry::from_file(&path, mod_metadata)
          {
            let hybrid = HybridPath::Temp(temp, source, Some(path));
            if &mod_info.version_checker.as_ref().unwrap().version == target_version {
              handle_delete(ext_ctx, mod_info, hybrid, old_path).await?;
            } else {
              ext_ctx
                .submit_command(
                  INSTALL,
                  ChannelMessage::Error(
                    mod_info.name.clone(),
                    "Downloaded version does not match expected version".to_string(),
                  ),
                  Target::Auto,
                )
                .expect("Send error over async channel");
            }
          } else {
            ext_ctx
              .submit_command(
                INSTALL,
                ChannelMessage::Error(mod_id.clone(), "Some kind of unpack error".to_string()),
                Target::Auto,
              )
              .expect("Send error over async channel");
          }
        }
        Err(err) => {
          println!("{err:?}");
          ext_ctx
            .submit_command(
              INSTALL,
              ChannelMessage::Error(mod_id.clone(), err.to_string()),
              Target::Auto,
            )
            .expect("Send error over async channel");
        }
      };
    }
    Err(err) => {
      ext_ctx
        .submit_command(
          INSTALL,
          ChannelMessage::Error(mod_id.clone(), err.to_string()),
          Target::Auto,
        )
        .expect("Send error over async channel");
    }
  }

  Ok(())
}

pub async fn download(
  url: String,
  ext_ctx: ExtEventSink,
) -> Result<tempfile::NamedTempFile, InstallError> {
  static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

  type UpdateBalancer =
    LoadBalancer<(i64, String, f64), Vec<(i64, String, f64)>, HashMap<i64, (i64, String, f64)>>;
  static UPDATE_BALANCER: UpdateBalancer = LoadBalancer::new(DOWNLOAD_PROGRESS);

  let mut file = tempfile::NamedTempFile::new()?;
  let client = reqwest::ClientBuilder::default()
    .redirect(reqwest::redirect::Policy::limited(200))
    .user_agent(APP_USER_AGENT)
    .build()?;

  let mut res = client.get(&url).send().await?;

  let name = res
    .headers()
    .get(reqwest::header::CONTENT_DISPOSITION)
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.rsplit_once("filename="))
    .map_or_else(
      || {
        Url::parse(&url)
          .ok()
          .and_then(|url| {
            url
              .path_segments()
              .and_then(std::iter::Iterator::last)
              .map(std::string::ToString::to_string)
          })
          .unwrap_or(url)
      },
      |(_, filename)| filename.to_string(),
    );

  let tx = UPDATE_BALANCER.sender(ext_ctx.clone());

  let start = Local::now().timestamp();
  let _ = ext_ctx.submit_command(DOWNLOAD_STARTED, (start, name.clone()), Target::Auto);

  let total = res.content_length();
  let mut current_total = 0.0;
  while let Some(chunk) = res.chunk().await? {
    file.write_all(&chunk)?;
    if let Some(total) = total {
      current_total += chunk.len() as f64;
      let _ = tx.send((start, name.clone(), (current_total / total as f64)));
    }
  }

  let _ = tx.send((start, name, 1.0)).inspect_err(|e| {
    eprintln!("err: {e:?}");
  });

  Ok(file)
}

#[derive(Debug, Clone, Data)]
pub enum HybridPath {
  PathBuf(#[data(eq)] PathBuf),
  Temp(Arc<TempDir>, String, #[data(eq)] Option<PathBuf>),
}

impl HybridPath {
  #[must_use]
  pub fn get_path_copy(&self) -> PathBuf {
    match self {
      HybridPath::PathBuf(ref path) | HybridPath::Temp(_, _, Some(ref path)) => path.clone(),
      HybridPath::Temp(ref arc, _, None) => arc.path().to_path_buf(),
    }
  }

  #[must_use]
  pub fn with_path(mut self, path: &PathBuf) -> Self {
    match &mut self {
      HybridPath::PathBuf(inner) => inner.clone_from(path),
      HybridPath::Temp(_, _, path_opt) => {
        path_opt.replace(path.clone());
      }
    };

    self
  }

  #[must_use]
  pub fn source(&self) -> Cow<str> {
    match self {
      HybridPath::PathBuf(path) => path.to_string_lossy(),
      HybridPath::Temp(_, source, _) => source.into(),
    }
  }
}

impl From<PathBuf> for HybridPath {
  fn from(value: PathBuf) -> Self {
    Self::PathBuf(value)
  }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
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

#[derive(Debug, Clone)]
pub enum ChannelMessage {
  /// New mod entry
  Success(ModEntry),
  Error(String, String),
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

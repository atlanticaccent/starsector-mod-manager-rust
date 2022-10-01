use std::{
  collections::{HashMap, VecDeque},
  fs::{copy, create_dir_all},
  io::{self, Write},
  iter::FusedIterator,
  path::{Path, PathBuf},
  sync::{Arc, LazyLock, Mutex, Weak},
};

use chrono::Local;
use druid::{ExtEventSink, Selector, SingleUse, Target};
use if_chain::if_chain;
use im::Vector;
use remove_dir_all::remove_dir_all;
use reqwest::Url;
use snafu::{OptionExt, ResultExt, Snafu};
use tempfile::{tempdir, TempDir};
use tokio::{
  fs::rename,
  select,
  sync::mpsc,
  task::{self, JoinSet},
  time::{sleep, timeout, Duration, Instant},
};

use crate::app::mod_entry::ModEntry;

use super::mod_entry::ModMetadata;

#[derive(Clone)]
pub enum Payload {
  Initial(Vec<PathBuf>),
  Resumed(Arc<ModEntry>, HybridPath, PathBuf),
  Download(Arc<ModEntry>),
}

pub const INSTALL: Selector<ChannelMessage> = Selector::new("install.message");
pub const DOWNLOAD_STARTED: Selector<(i64, String)> = Selector::new("install.download.started");
pub const DOWNLOAD_PROGRESS: Selector<Vec<(i64, String, f64)>> =
  Selector::new("install.download.progress");
pub const INSTALL_ALL: Selector<SingleUse<(Vector<PathBuf>, HybridPath)>> =
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
        handles.spawn(async move { handle_delete(ext_ctx.clone(), entry, path, existing).await });
      }
      Payload::Download(entry) => {
        handles.spawn(handle_auto(ext_ctx, entry));
      }
    }
    while handles.join_next().await.is_some() {}
  }
}

async fn handle_path(
  ext_ctx: ExtEventSink,
  path: PathBuf,
  mods_dir: Arc<PathBuf>,
  installed: Arc<Vec<String>>,
) {
  let file_name = path
    .file_name()
    .map(|f| f.to_string_lossy().to_string())
    .unwrap_or_else(|| String::from("unknown"));

  let mod_folder = if path.is_file() {
    let decompress = task::spawn_blocking(move || decompress(path))
      .await
      .expect("Run decompression");
    match decompress {
      Ok(temp) => HybridPath::Temp(Arc::new(temp), file_name.clone(), None),
      Err(err) => {
        println!("{:?}", err);
        ext_ctx
          .submit_command(
            INSTALL,
            ChannelMessage::Error(file_name, err.to_string()),
            Target::Auto,
          )
          .expect("Send error over async channel");

        return;
      }
    }
  } else {
    HybridPath::PathBuf(path)
  };

  let dir = mod_folder.get_path_copy();
  match timeout(
    std::time::Duration::from_millis(500),
    task::spawn_blocking(move || {
      ModSearch::new(&dir).exhaustive().context(Io {
        detail: "IO error searching for mods",
      })
    }),
  )
  .await
  .context(Timeout)
  .and_then(|res| res.context(Join))
  .flatten()
  {
    Ok(mod_paths) => {
      if mod_paths.len() > 1 {
        let _ = ext_ctx.submit_command(
          INSTALL,
          ChannelMessage::FoundMultiple(mod_folder, mod_paths),
          Target::Auto,
        );
      } else if let Some(mod_path) = mod_paths.get(0)
          && let mod_metadata = ModMetadata::new()
          && mod_metadata.save(mod_path).await.is_ok()
          && let Ok(mut mod_info) = ModEntry::from_file(mod_path, mod_metadata)
        {
          let rewrite = || {
            match mod_folder {
              HybridPath::PathBuf(_) => HybridPath::PathBuf(mod_path.clone()),
              HybridPath::Temp(temp, _file_name, _) => HybridPath::Temp(temp, _file_name, Some(mod_path.clone()))
            }
          };
          if let Some(id) = installed.iter().find(|existing| **existing == mod_info.id) {
            // note: this is probably the way wrong way of doing this
            // instead, just submit the new entry if it doesn't conflict with an existing path, _then_ detect the conflict
            // that way there's less chance an existing ID gets missed due to the ID list effectively getting cached when
            // this function starts
            ext_ctx.submit_command(INSTALL, ChannelMessage::Duplicate(id.clone().into(), rewrite(), Arc::new(mod_info)), Target::Auto).expect("Send query over async channel");
          } else if mods_dir.join(mod_info.id.clone()).exists() {
            let mod_folder = rewrite();
            ext_ctx.submit_command(INSTALL, ChannelMessage::Duplicate(mods_dir.join(mod_info.id.clone()).into(), mod_folder, Arc::new(mod_info)), Target::Auto).expect("Send query over async channel");
          } else {
            move_or_copy(mod_path.clone(), mods_dir.join(&mod_info.id)).await;

            mod_info.set_path(mods_dir.join(&mod_info.id));
            ext_ctx.submit_command(INSTALL, ChannelMessage::Success(Arc::new(mod_info)), Target::Auto).expect("Send success over async channel");
          }
        } else {
          ext_ctx.submit_command(INSTALL, ChannelMessage::Error(file_name, "Could not find mod folder or parse mod_info file.".to_string()), Target::Auto).expect("Send error over async channel");
        }
    }
    Err(err) => {
      ext_ctx
        .submit_command(
          INSTALL,
          ChannelMessage::Error(file_name, format!("Failed to find mod, err: {}", err)),
          Target::Auto,
        )
        .expect("Send error over async channel");
    }
  }
}

pub fn decompress(path: PathBuf) -> Result<TempDir, InstallError> {
  let source = std::fs::File::open(&path).context(Io {
    detail: "Failed to open source archive",
  })?;
  let temp_dir = tempdir().context(Io {
    detail: "Failed to open a temp dir",
  })?;
  let mime_type = infer::get_from_path(&path)
    .context(Io {
      detail: "Failed to open archive for archive type inference",
    })?
    .context(Mime {
      detail: "Failed to get mime type",
    })?
    .mime_type();

  match mime_type {
    "application/vnd.rar" | "application/x-rar-compressed" => {
      #[cfg(not(target_env = "musl"))]
      unrar::Archive::new(path.to_string_lossy().to_string())
        .extract_to(temp_dir.path().to_string_lossy().to_string())
        .ok()
        .context(Unrar {
          detail: "Opaque Unrar error. Assume there's been an error unpacking your rar archive.",
        })?
        .process()
        .ok()
        .context(Unrar {
          detail: "Opaque Unrar error. Assume there's been an error unpacking your rar archive.",
        })?;
      // trust me I tried to de-dupe this and it's buggered
      #[cfg(target_env = "musl")]
      compress_tools::uncompress_archive(source, temp_dir.path(), compress_tools::Ownership::Ignore)
        .context(CompressTools {})?
    }
    _ => {
      compress_tools::uncompress_archive(source, temp_dir.path(), compress_tools::Ownership::Ignore)
        .context(CompressTools {})?
    }
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
      if path.join("mod_info.json").is_file() {
        return Some(Ok(path));
      } else if path.is_dir() {
        let res: std::io::Result<()> = try {
          for entry in path.read_dir()? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
              self.paths.push_back(entry.path());
            }
          }
        };

        if let Err(err) = res {
          return Some(Err(err));
        }
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
      copy(entry.path(), &to.join(entry.file_name()))?;
    }
  }

  Ok(())
}

async fn handle_delete(
  ext_ctx: ExtEventSink,
  mut entry: Arc<ModEntry>,
  new_path: HybridPath,
  old_path: PathBuf,
) {
  let destination = old_path.canonicalize().expect("Canonicalize destination");
  remove_dir_all(destination).expect("Remove old mod");

  let origin = new_path.get_path_copy();
  move_or_copy(origin, old_path.clone()).await;
  (*Arc::make_mut(&mut entry)).set_path(old_path);

  ext_ctx
    .submit_command(INSTALL, ChannelMessage::Success(entry), Target::Auto)
    .expect("Send success over async channel");
}

async fn handle_auto(ext_ctx: ExtEventSink, entry: Arc<ModEntry>) {
  let url = entry
    .remote_version
    .as_ref()
    .unwrap()
    .direct_download_url
    .as_ref()
    .unwrap();
  let target_version = &entry.remote_version.as_ref().unwrap().version;
  match download(url.clone(), ext_ctx.clone()).await {
    Ok(file) => {
      let path = file.path().to_path_buf();
      let decompress = task::spawn_blocking(move || decompress(path))
        .await
        .expect("Run decompression");
      match decompress {
        Ok(temp) => {
          let temp = Arc::new(temp);
          let path = temp.path().to_owned();
          let source = url.clone();
          if_chain! {
            if let Ok(Some(path)) = task::spawn_blocking(move || ModSearch::new(&path).first())
              .await
              .expect("Run blocking search")
              .context(Io { detail: "File IO error when searching for mod" });
            let mod_metadata = ModMetadata::new();
            if mod_metadata.save(&path).await.is_ok();
            if let Ok(mod_info) = ModEntry::from_file(&path, mod_metadata);
            then {
              let hybrid = HybridPath::Temp(temp, source, Some(path));
              if &mod_info.version_checker.as_ref().unwrap().version != target_version {
                ext_ctx.submit_command(INSTALL, ChannelMessage::Error(mod_info.name.clone(), "Downloaded version does not match expected version".to_string()), Target::Auto).expect("Send error over async channel");
              } else {
                handle_delete(ext_ctx, Arc::new(mod_info), hybrid, entry.path.clone()).await;
              }
            } else {
              ext_ctx.submit_command(INSTALL, ChannelMessage::Error(entry.id.clone(), "Some kind of unpack error".to_string()), Target::Auto).expect("Send error over async channel");
            }
          }
        }
        Err(err) => {
          println!("{:?}", err);
          ext_ctx
            .submit_command(
              INSTALL,
              ChannelMessage::Error(entry.id.clone(), err.to_string()),
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
          ChannelMessage::Error(entry.id.clone(), err.to_string()),
          Target::Auto,
        )
        .expect("Send error over async channel");
    }
  }
}

pub async fn download(
  url: String,
  ext_ctx: ExtEventSink,
) -> Result<tempfile::NamedTempFile, InstallError> {
  static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

  static UPDATE_BALANCER: LazyLock<Mutex<Weak<mpsc::UnboundedSender<(i64, String, f64)>>>> =
    LazyLock::new(|| Mutex::new(Weak::new()));

  let mut file = tempfile::NamedTempFile::new().context(Io {
    detail: String::from("Failed to create named temp file to write to"),
  })?;
  let client = reqwest::ClientBuilder::default()
    .redirect(reqwest::redirect::Policy::limited(200))
    .user_agent(APP_USER_AGENT)
    .build()
    .context(Network {})?;

  let mut res = client.get(&url).send().await.context(Network {})?;

  let name = res
    .headers()
    .get(reqwest::header::CONTENT_DISPOSITION)
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.rsplit_once("filename="))
    .map(|(_, filename)| filename.to_string())
    .unwrap_or_else(|| {
      Url::parse(&url)
        .ok()
        .and_then(|url| {
          url
            .path_segments()
            .and_then(|segments| segments.last())
            .map(|s| s.to_string())
        })
        .unwrap_or(url)
    });

  let tx = {
    let mut sender = UPDATE_BALANCER.lock().unwrap();
    if let Some(tx) = sender.upgrade() {
      tx
    } else {
      let (tx, mut rx) = mpsc::unbounded_channel::<(i64, String, f64)>();
      let ext_ctx = ext_ctx.clone();
      let tx = Arc::new(tx);
      *sender = Arc::downgrade(&tx);
      task::spawn(async move {
        let sleep = sleep(Duration::from_millis(50));
        tokio::pin!(sleep);

        let mut queue: HashMap<i64, (i64, String, f64)> = HashMap::new();
        loop {
          select! {
            message = rx.recv() => {
              match message {
                Some(message) => {
                  queue.insert(message.0, message);
                },
                None => {
                  if !queue.is_empty() {
                    let vals: Vec<(i64, String, f64)> = queue.drain().map(|(_, val)| val).collect();
                    let _ = ext_ctx.submit_command(DOWNLOAD_PROGRESS, vals, Target::Auto);
                  }
                  break
                },
              }
            },
            _ = &mut sleep => {
              let vals: Vec<(i64, String, f64)> = queue.drain().map(|(_, val)| val).collect();
              let _ = ext_ctx.submit_command(DOWNLOAD_PROGRESS, vals, Target::Auto);
              sleep.as_mut().reset(Instant::now() + Duration::from_millis(50));
            }
          }
        }
      });

      tx
    }
  };

  let start = Local::now().timestamp();
  let _ = ext_ctx.submit_command(DOWNLOAD_STARTED, (start, name.clone()), Target::Auto);

  let total = res.content_length();
  let mut current_total = 0.0;
  while let Some(chunk) = res.chunk().await.context(Network {})? {
    file.write(&chunk).context(Io {
      detail: String::from("Failed to write downloaded chunk to temp file"),
    })?;
    if let Some(total) = total {
      current_total += chunk.len() as f64;
      let _ = tx.send((start, name.clone(), (current_total / total as f64)));
    }
  }

  let _ = tx.send((start, name, 1.0)).inspect_err(|e| {
    eprintln!("err: {:?}", e);
  });

  Ok(file)
}

#[derive(Debug, Clone)]
pub enum HybridPath {
  PathBuf(PathBuf),
  Temp(Arc<TempDir>, String, Option<PathBuf>),
}

impl HybridPath {
  pub fn get_path_copy(&self) -> PathBuf {
    match self {
      HybridPath::PathBuf(ref path) => path.clone(),
      HybridPath::Temp(_, _, Some(ref path)) => path.clone(),
      HybridPath::Temp(ref arc, _, None) => arc.path().to_path_buf(),
    }
  }
}

#[derive(Debug, Snafu)]
pub enum InstallError {
  Io {
    source: std::io::Error,
    detail: String,
  },
  Mime {
    detail: String,
  },
  CompressTools {
    source: compress_tools::Error,
  },
  Unrar {
    detail: String,
  },
  Network {
    source: reqwest::Error,
  },
  Timeout {
    source: tokio::time::error::Elapsed,
  },
  Join {
    source: tokio::task::JoinError,
  },
  Any {
    detail: String,
  },
}

#[derive(Debug, Clone)]
pub enum ChannelMessage {
  /// New mod entry
  Success(Arc<ModEntry>),
  /// ID, Conflicting ID or Path, Path to new, New Mod Entry
  Duplicate(StringOrPath, HybridPath, Arc<ModEntry>),
  FoundMultiple(HybridPath, Vec<PathBuf>),
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
  use std::fs;

  use self_update::TempDir;
  use tempfile::tempdir;

  use super::ModSearch;

  fn create_folder_with_n_mods<const N: usize>() -> TempDir {
    let temp_dir = tempdir().expect("Create temp dir");

    for i in 0..N {
      fs::create_dir(temp_dir.path().join(format!("{}", i))).expect("Create fake mod dir");
      fs::File::create(temp_dir.path().join(format!("{}", i)).join("mod_info.json"))
        .expect("Create fake mod_info.json");
    }

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

    for i in 0..5 {
      assert_eq!(
        iter
          .next()
          .transpose()
          .ok()
          .flatten()
          .unwrap_or_else(|| panic!("Failed to find mod {}", i)),
        mods_dir.path().join(i.to_string())
      );
    }

    assert!(iter.next().is_none());
  }
}

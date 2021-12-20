use std::path::PathBuf;
use std::hash::{Hash, Hasher};
use std::{
  fs::{copy, create_dir_all, read_dir},
  io, io::Write
};
use std::sync::Arc;
use iced_futures::futures::{self, future, StreamExt};
use tokio::{
  sync::mpsc,
  task,
  fs::rename
};
use compress_tools;
use tempfile::{tempdir, TempDir};
use snafu::{Snafu, ResultExt, OptionExt};
// use find_mountpoint::find_mountpoint;
use remove_dir_all::remove_dir_all;
use infer;
use unrar;
use if_chain::if_chain;

use super::mod_list::ModEntry;

#[derive(Clone)]
pub struct Installation<I> 
where
  I: 'static + Hash + Copy + Send,
{
  pub id: I,
  payload: Payload,
  mods_dir: PathBuf,
  installed: Vec<String>
}

#[derive(Clone)]
pub enum Payload {
  Initial(Vec<PathBuf>),
  Resumed(String, HybridPath, PathBuf),
  Download(String, String, PathBuf)
}

impl From<Vec<PathBuf>> for Payload {
  fn from(from: Vec<PathBuf>) -> Self {
    Payload::Initial(from)
  }
}

impl From<(String, HybridPath, PathBuf)> for Payload {
  fn from((name, new_path, old_path) : (String, HybridPath, PathBuf)) -> Self {
    Payload::Resumed(name, new_path, old_path)
  }
}

impl From<(String, String, PathBuf)> for Payload {
  fn from((url, target_version, old_path): (String, String, PathBuf)) -> Self {
    Payload::Download(url, target_version, old_path)
  }
}

impl<I> Installation<I> 
where
  I: 'static + Hash + Copy + Send,
{
  pub fn new<T: Into<Payload>>(id: I, payload: T, mods_dir: PathBuf, installed: Vec<String>) -> Self {
    Installation {
      id,
      payload: payload.into(),
      mods_dir,
      installed
    }
  }

  pub fn install(self) -> iced::Subscription<Progress<I>> {
    iced::Subscription::from_recipe(self)
  }
}

// Make sure iced can use our download stream
impl<H, I, T> iced_native::subscription::Recipe<H, I> for Installation<T>
where
  T: 'static + Hash + Copy + Send,
  H: Hasher,
{
  type Output = Progress<T>;

  fn hash(&self, state: &mut H) {
    struct Marker;
    std::any::TypeId::of::<Marker>().hash(state);

    self.id.hash(state);
  }

  fn stream(
    self: Box<Self>,
    _input: futures::stream::BoxStream<'static, I>,
  ) -> futures::stream::BoxStream<'static, Self::Output> {
    let id = self.id;

    Box::pin(futures::stream::unfold(
      State::Ready(self.payload, self.mods_dir, self.installed),
      move |state| async move {
        match state {
          State::Ready(payload, mods_dir, installed) => {
            let (tx, rx) = mpsc::unbounded_channel();

            match payload {
              Payload::Initial(paths) => {
                for path in paths {
                  let task_tx = tx.clone();
                  let mods_dir = mods_dir.clone();
                  let installed = installed.clone();
  
                  tokio::spawn(async move {
                    handle_path(task_tx, path, mods_dir, installed).await;
                  });
                }
              },
              Payload::Resumed(name, new_path, old_path) => {
                tokio::spawn(async move {
                  handle_delete(tx, name, new_path, old_path).await;
                });
              },
              Payload::Download(url, target_version, old_path) => {
                tokio::spawn(async move {
                  handle_auto(tx, url, target_version, old_path, mods_dir).await;
                });
              }
            }

            Some((
              None,
              State::Installing {
                receiver: rx,
                complete: vec![],
                errored: vec![]
              }
            ))
          },
          State::Installing {
            mut receiver,
            mut complete,
            mut errored
          } => match receiver.recv().await {
            Some(ChannelMessage::Success(mod_name)) => {
              complete.push(mod_name);

              Some((
                Some(Progress::Finished),
                State::Installing {
                  receiver,
                  complete,
                  errored
                }
              ))
            },
            Some(ChannelMessage::Duplicate(name, id, new_path, old_path)) => {
              Some((
                Some(Progress::Query(name, id, new_path, old_path)),
                State::Installing {
                  receiver,
                  complete,
                  errored
                }
              ))
            },
            Some(ChannelMessage::Error(mod_name)) => {
              errored.push(mod_name);

              Some((
                None,
                State::Installing {
                  receiver,
                  complete,
                  errored
                }
              ))
            },
            None => {
              Some((
                Some(Progress::Completed(id, complete, errored)),
                State::Finished
              ))
            }
          },
          State::Finished => {
            None
          }
        }
      },
    ).filter_map(|prog| future::ready(prog)))
  }
}

async fn handle_path(tx: mpsc::UnboundedSender<ChannelMessage>, path: PathBuf, mods_dir: PathBuf, installed: Vec<String>) {
  let mut mod_folder = if path.is_file() {
    let decompress = task::spawn_blocking(move || decompress(path)).await.expect("Run decompression");
    match decompress {
      Ok(temp) => HybridPath::Temp(Arc::new(temp), None),
      Err(err) => {
        println!("{:?}", err);
        tx.send(ChannelMessage::Error(err.to_string())).expect("Send error over async channel");

        return;
      }
    }
  } else {
    HybridPath::PathBuf(path)
  };

  let dir = mod_folder.get_path_copy();
  if_chain! {
    if let Ok(maybe_path) = task::spawn_blocking(move || find_nested_mod(&dir)).await.expect("Find mod in given folder");
    if let Some(mod_path) = maybe_path;
    if let Ok(mod_info) = ModEntry::from_file(mod_path.join("mod_info.json"));
    then {
      if let Some(id) = installed.into_iter().find(|existing| **existing == mod_info.id) {
        mod_folder = match mod_folder {
          HybridPath::PathBuf(_) => HybridPath::PathBuf(mod_path.clone()),
          HybridPath::Temp(temp, _) => HybridPath::Temp(temp, Some(mod_path.clone()))
        };

        tx.send(ChannelMessage::Duplicate(mod_info.name, id, mod_folder, None)).expect("Send query over async channel");
      } else if !mods_dir.join(mod_info.id.clone()).exists() {
        move_or_copy(mod_path, mods_dir.join(mod_info.id)).await;

        tx.send(ChannelMessage::Success(mod_info.name)).expect("Send success over async channel");
      } else {
        mod_folder = match mod_folder {
          HybridPath::PathBuf(_) => HybridPath::PathBuf(mod_path.clone()),
          HybridPath::Temp(temp, _) => HybridPath::Temp(temp, Some(mod_path.clone()))
        };

        tx.send(ChannelMessage::Duplicate(mod_info.name, String::new(), mod_folder, Some(mods_dir.join(mod_info.id.clone())))).expect("Send query over async channel");
      }
    } else {
      tx.send(ChannelMessage::Error(format!("Could not find mod folder or parse mod_info file."))).expect("Send error over async channel");
    }
  }
}

fn decompress(path: PathBuf) -> Result<TempDir, InstallError> {
  let source = std::fs::File::open(&path).context(Io {})?;
  let temp_dir = tempdir().context(Io {})?;
  let mime_type = infer::get_from_path(&path)
    .context(Io {})?
    .context(Mime { detail: "Failed to get mime type"})?
    .mime_type();

  match mime_type {
    "application/vnd.rar" | "application/x-rar-compressed" => {
      #[cfg(not(target_env="musl"))]
      unrar::Archive::new(path.to_string_lossy().to_string())
        .extract_to(temp_dir.path().to_string_lossy().to_string())
        .ok().context(Unrar { detail: "Opaque Unrar error. Assume there's been an error unpacking your rar archive." })?
        .process()
        .ok().context(Unrar { detail: "Opaque Unrar error. Assume there's been an error unpacking your rar archive." })?;
        // trust me I tried to de-dupe this and it's buggered
      #[cfg(target_env="musl")]
      compress_tools::uncompress_archive(source, temp_dir.path(), compress_tools::Ownership::Preserve).context(CompressTools {})?
    }
    _ => {
      compress_tools::uncompress_archive(source, temp_dir.path(), compress_tools::Ownership::Preserve).context(CompressTools {})?
    }
  }

  Ok(temp_dir)
}

fn find_nested_mod(dest: &PathBuf) -> std::io::Result<Option<PathBuf>> {
  for entry in read_dir(dest)? {
    let entry = entry?;
    if entry.file_type()?.is_dir() {
      let res = find_nested_mod(&entry.path())?;
      if res.is_some() {
        return Ok(res);
      }
    } else if entry.file_type()?.is_file() {
      if entry.file_name() == "mod_info.json" {
        return Ok(Some(dest.to_path_buf()));
      }
    }
  }

  Ok(None)
}

async fn move_or_copy(from: PathBuf, to: PathBuf) {
  // let mount_from = find_mountpoint(&from).expect("Find origin mount point");
  // let mount_to = find_mountpoint(&to).expect("Find destination mount point");

  if let Err(_) = rename(from.clone(), to.clone()).await {
    task::spawn_blocking(move || copy_dir_recursive(&to, &from)).await
      .expect("Run blocking dir copy")
      .expect("Copy dir to new destination");
  }
}

fn copy_dir_recursive(to: &PathBuf, from: &PathBuf) -> io::Result<()> {
  if !to.exists() {
    create_dir_all(to)?;
  }

  for entry in from.read_dir()? {
    let entry = entry?;
    if entry.file_type()?.is_dir() {
      copy_dir_recursive(&to.to_path_buf().join(entry.file_name()), &entry.path())?;
    } else if entry.file_type()?.is_file() {
      copy(entry.path(), &to.to_path_buf().join(entry.file_name()))?;
    }
  }

  Ok(())
}

async fn handle_delete(tx: mpsc::UnboundedSender<ChannelMessage>, name: String, new_path: HybridPath, old_path: PathBuf) {
  let destination = old_path.canonicalize().expect("Canonicalize destination");
  remove_dir_all(destination).expect("Remove old mod");

  let origin = new_path.get_path_copy();
  move_or_copy(origin, old_path).await;

  tx.send(ChannelMessage::Success(name)).expect("Send success over async channel");
}

async fn handle_auto(tx: mpsc::UnboundedSender<ChannelMessage>, url: String, target_version: String, old_path: PathBuf, _: PathBuf) {
  match download(url).await {
    Ok(file) => {
      let path = file.path().to_path_buf();
      let decompress = task::spawn_blocking(move || decompress(path)).await.expect("Run decompression");
      match decompress {
        Ok(temp) => {
          let hybrid = HybridPath::Temp(Arc::new(temp), None);
          let path = hybrid.get_path_copy();
          if_chain! {
            if let Ok(Some(path)) = task::spawn_blocking(move || find_nested_mod(&path)).await.expect("Run blocking search").context(Io {});
            if let Ok(mod_info) = ModEntry::from_file(path.join("mod_info.json"));
            then {
              let hybrid = if let HybridPath::Temp(temp, _) = hybrid {
                HybridPath::Temp(temp, Some(path))
              } else {
                unreachable!()
              };
              if mod_info.version.to_string() != target_version {
                tx.send(ChannelMessage::Error(format!("Downloaded version does not match expected version"))).expect("Send error over async channel");
              } else {
                handle_delete(tx, mod_info.name, hybrid, old_path).await;
              }
            } else {
              tx.send(ChannelMessage::Error(format!("Some kind of unpack error"))).expect("Send error over async channel");
            }
          }
        },
        Err(err) => {
          println!("{:?}", err);
          tx.send(ChannelMessage::Error(err.to_string())).expect("Send error over async channel");

          return;
        }
      };
    },
    Err(err) => {
      tx.send(ChannelMessage::Error(err.to_string())).expect("Send error over async channel");
    }
  }
}

async fn download(url: String) -> Result<tempfile::NamedTempFile, InstallError> {
  let mut file = tempfile::NamedTempFile::new().context(Io {})?;
  let mut res = reqwest::get(url).await.context(Network {})?;

  while let Some(chunk) = res.chunk().await.context(Network {})? {
    file.write(&chunk).context(Io {})?;
  };

  Ok(file)
}

#[derive(Debug, Clone)]
pub enum HybridPath {
  PathBuf(PathBuf),
  Temp(Arc<TempDir>, Option<PathBuf>),
}

impl HybridPath {
  fn get_path_copy(&self) -> PathBuf {
    match self {
      HybridPath::PathBuf(ref path) => path.clone(),
      HybridPath::Temp(_, Some(ref path)) => path.clone(),
      HybridPath::Temp(ref arc, None) => arc.path().to_path_buf()
    }
  }
}

#[derive(Debug, Snafu)]
enum InstallError {
  Io { source: std::io::Error },
  Mime { detail: String },
  CompressTools { source: compress_tools::Error },
  Unrar { detail: String },
  Network { source: reqwest::Error },
  Any { detail: String }
}

#[derive(Debug, Clone)]
pub enum Progress<I> 
where
  I: 'static + Hash + Copy + Send,
{
  Completed(I, Vec<String>, Vec<String>),
  Query(String, String, HybridPath, Option<PathBuf>),
  Finished
}

pub enum State {
  Ready(Payload, PathBuf, Vec<String>),
  Installing {
    receiver: mpsc::UnboundedReceiver<ChannelMessage>,
    complete: Vec<String>,
    errored: Vec<String>
  },
  Finished
}

#[derive(Debug, Clone)]
pub enum ChannelMessage {
  Success(String),
  Duplicate(String, String, HybridPath, Option<PathBuf>),
  Error(String)
}

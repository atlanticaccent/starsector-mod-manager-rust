use std::path::PathBuf;
use std::hash::{Hash, Hasher};
use std::{
  fs::{copy, create_dir_all, read_dir},
  io
};
use std::sync::Arc;
use iced_futures::futures::{self, future, StreamExt};
use tokio::{
  sync::mpsc,
  task,
  fs::rename
};
use libarchive;
use tempfile::{tempdir_in, TempDir};
use snafu::{Snafu, ResultExt};
// use find_mountpoint::find_mountpoint;

use super::mod_list::ModEntry;

// Just a little utility function
pub fn install<I: 'static + Hash + Copy + Send>(
  id: I,
  paths: Vec<PathBuf>,
  mods_dir: PathBuf,
  installed: Vec<String>
) -> iced::Subscription<Progress> {
  iced::Subscription::from_recipe(Installation {
    id,
    payload: Payload::Initial(paths),
    mods_dir,
    installed
  })
}

pub fn resume<I: 'static + Hash + Copy + Send>(
  id: I,
  resumed_id: String, 
  resumed_path: PathBuf,
  mods_dir: PathBuf,
  installed: Vec<String>
) -> iced::Subscription<Progress> {
  iced::Subscription::from_recipe(Installation {
    id,
    payload: Payload::Resumed(resumed_id, resumed_path),
    mods_dir,
    installed
  })
}

pub struct Installation<I> {
  id: I,
  payload: Payload,
  mods_dir: PathBuf,
  installed: Vec<String>
}

pub enum Payload {
  Initial(Vec<PathBuf>),
  Resumed(String, PathBuf)
}

// Make sure iced can use our download stream
impl<H, I, T> iced_native::subscription::Recipe<H, I> for Installation<T>
where
  T: 'static + Hash + Copy + Send,
  H: Hasher,
{
  type Output = Progress;

  fn hash(&self, state: &mut H) {
    struct Marker;
    std::any::TypeId::of::<Marker>().hash(state);

    self.id.hash(state);
  }

  fn stream(
    self: Box<Self>,
    _input: futures::stream::BoxStream<'static, I>,
  ) -> futures::stream::BoxStream<'static, Self::Output> {
    Box::pin(futures::stream::unfold(
      State::Ready(self.payload, self.mods_dir, self.installed),
      move |state| async move {
        match state {
          State::Ready(payload, mods_dir, installed) => {
            let (tx, rx) = mpsc::unbounded_channel();

            async {
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
                Payload::Resumed(id, path) => {
                  
                }
              }
            }.await;

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
            Some(ChannelMessage::Success(mod_id)) => {
              complete.push(mod_id);

              Some((
                None,
                State::Installing {
                  receiver,
                  complete,
                  errored
                }
              ))
            },
            Some(ChannelMessage::Duplicate(name, id, path)) => {
              Some((
                Some(Progress::Query(name, id, path)),
                State::Installing {
                  receiver,
                  complete,
                  errored
                }
              ))
            },
            Some(ChannelMessage::Error(mod_id)) => {
              errored.push(mod_id.clone());

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
                Some(Progress::Finished(complete, errored)),
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
    let dir = mods_dir.clone();
    let decompress = task::spawn_blocking(move || decompress(path, dir)).await.expect("Run decompression");
    match decompress {
      Ok(temp) => HybridPath::Temp(Arc::new(temp), None),
      Err(err) => {
        tx.send(ChannelMessage::Error(err.to_string())).expect("Send error over async channel");

        return;
      }
    }
  } else {
    HybridPath::PathBuf(path)
  };

  let dir: PathBuf = mod_folder.as_ref().into();
  if let Ok(maybe_path) = task::spawn_blocking(move || find_nested_mod(&dir)).await.expect("Find mod in given folder") {
    if let Some(mod_path) = maybe_path {
      if let Ok(mod_info) = ModEntry::from_file(mod_path.join("mod_info.json")) {
        if let Some(id) = installed.into_iter().find(|existing| **existing == mod_info.id) {
          mod_folder = match mod_folder {
            HybridPath::PathBuf(_) => HybridPath::PathBuf(mod_path.clone()),
            HybridPath::Temp(temp, _) => HybridPath::Temp(temp, Some(mod_path.clone()))
          };

          tx.send(ChannelMessage::Duplicate(mod_info.name, id, mod_folder)).expect("Send query over async channel");
        } else {
          move_or_copy(mod_path, mods_dir.join(mod_info.id.clone())).await;

          tx.send(ChannelMessage::Success(mod_info.id)).expect("Send success over async channel");
        }
      }
    }
  }
}

fn decompress(path: PathBuf, mods_dir: PathBuf) -> Result<TempDir, InstallError> {
  let temp_dir = tempdir_in(mods_dir).context(Io {})?;

  let mut builder = libarchive::reader::Builder::new();

  // builder.support_compression(libarchive::archive::ReadCompression::All).context(Libarchive)?;
  builder.support_format(libarchive::archive::ReadFormat::All).context(Libarchive)?;
  builder.support_filter(libarchive::archive::ReadFilter::All).context(Libarchive)?;

  let mut reader = builder.open_file(path).context(Libarchive)?;

  let mut writer = libarchive::writer::Disk::new();
  let output_dir = temp_dir.path();

  writer.write(&mut reader, Some(&output_dir.to_owned().to_string_lossy())).context(Libarchive)?;

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

#[derive(Debug, Clone)]
pub enum HybridPath {
  PathBuf(PathBuf),
  Temp(Arc<TempDir>, Option<PathBuf>)
}

impl Into<PathBuf> for &HybridPath {
  fn into(self) -> PathBuf {
    match self {
      HybridPath::PathBuf(ref path) => path.clone(),
      HybridPath::Temp(ref temp, _) => temp.path().to_path_buf()
    }
  }
}

impl AsRef<HybridPath> for HybridPath {
  fn as_ref(&self) -> &HybridPath {
    &self
  }
}

#[derive(Debug, Snafu)]
enum InstallError {
  Io { source: std::io::Error },
  Libarchive { source: libarchive::error::ArchiveError },
  Any { detail: String }
}

#[derive(Debug, Clone)]
pub enum Progress {
  Finished(Vec<String>, Vec<String>),
  Query(String, String, HybridPath),
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
  Duplicate(String, String, HybridPath),
  Error(String)
}

use std::{any::Any, path::PathBuf};

use directories::ProjectDirs;
use druid::{ExtEventError, ExtEventSink, Selector, Target};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub enum InstallType {
  Uri(String),
  Path(PathBuf),
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
pub enum WebviewMessage {
  Navigation(String),
  Download(String),
  Shutdown,
  BlobFile(PathBuf),
  Maximize,
  Minimize,
}

#[derive(Debug)]
pub enum UserEvent {
  Navigation(String),
  NewWindow(String),
  AskDownload(String),
  Download(String),
  CancelDownload,
  BlobReceived(String),
  BlobChunk(Option<String>),
  PageLoaded,
}

lazy_static! {
  pub static ref PROJECT: ProjectDirs =
    ProjectDirs::from("org", "laird", "Starsector Mod Manager").expect("Get project dirs");
}

pub const FRACTAL_INDEX: &str = "https://fractalsoftworks.com/forum/index.php?topic=177.0";
pub const FRACTAL_MODS_FORUM: &str = "https://fractalsoftworks.com/forum/index.php?board=8.0";
pub const FRACTAL_MODDING_SUBFORUM: &str = "https://fractalsoftworks.com/forum/index.php?board=3.0";

pub const WEBVIEW_EVENT: Selector<UserEvent> = Selector::new("webview.event");
pub const WEBVIEW_INSTALL: Selector<InstallType> = Selector::new("webview.install");

pub const WEBVIEW_OFFSET: i16 = 34;

pub trait ExtEventSinkExt {
  fn submit_command_global<T: Any + Send>(
    &self,
    selector: Selector<T>,
    payload: impl Into<Box<T>>,
  ) -> Result<(), ExtEventError>;
}

impl ExtEventSinkExt for ExtEventSink {
  fn submit_command_global<T: Any + Send>(
    &self,
    selector: Selector<T>,
    payload: impl Into<Box<T>>,
  ) -> Result<(), ExtEventError> {
    self.submit_command(selector, payload, Target::Global)
  }
}

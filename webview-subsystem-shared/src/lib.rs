use std::path::PathBuf;

use const_format::concatcp;
use directories::ProjectDirs;
use interprocess::local_socket::LocalSocketStream;
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

lazy_static! {
  pub static ref PROJECT: ProjectDirs =
    ProjectDirs::from("org", "laird", "Starsector Mod Manager").expect("Get project dirs");
}

pub const PARENT_CHILD_PATH: &str = "/tmp/moss_parent.sock";
pub const PARENT_CHILD_SOCKET: &str = concatcp!("@", PARENT_CHILD_PATH);
pub const CHILD_PARENT_PATH: &str = "/tmp/moss_child.sock";
pub const CHILD_PARENT_SOCKET: &str = concatcp!("@", CHILD_PARENT_PATH);

pub fn handle_error(conn: std::io::Result<LocalSocketStream>) -> Option<LocalSocketStream> {
  match conn {
    Ok(val) => Some(val),
    Err(error) => {
      eprintln!("Incoming connection failed: {}", error);
      None
    }
  }
}

pub fn connect_parent() -> std::io::Result<LocalSocketStream> {
  LocalSocketStream::connect(PARENT_CHILD_SOCKET)
}

pub fn connect_child() -> std::io::Result<LocalSocketStream> {
  LocalSocketStream::connect(CHILD_PARENT_SOCKET)
}

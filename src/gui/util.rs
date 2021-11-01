use if_chain::if_chain;
use json_comments::strip_comments;
use std::{io::Read, path::PathBuf};

#[cfg_attr(target_os = "macos", path = "dialogs/macos.rs")]
#[cfg_attr(not(target_os = "macos"), path = "dialogs/other.rs")]
mod dialogs;
pub use self::dialogs::*;

use crate::gui::mod_list::ModVersionMeta;

pub fn error<T: AsRef<str>>(message: T) {
  Dialog::error(String::from(message.as_ref()));
}

pub fn notif<T: AsRef<str>>(message: T) {
  Dialog::notif(String::from(message.as_ref()));
}

pub fn query<T: AsRef<str>>(message: T) -> bool {
  Dialog::query(String::from(message.as_ref()))
}

pub fn select_folder_dialog(title: &str, path: &str) -> Option<PathBuf> {
  Dialog::select_folder(title, path)
}

pub fn select_archives(path: &str) -> Option<Vec<PathBuf>> {
  Dialog::select_archives(path)
}

pub async fn get_master_version(local: ModVersionMeta) -> (String, Result<Option<ModVersionMeta>, String>) {
  let res = send_request(local.remote_url.clone()).await;

  match res {
    Err(err) => (local.id, Err(err)),
    Ok(remote) => {
      if_chain! {
        let mut stripped = String::new();
        if strip_comments(remote.as_bytes()).read_to_string(&mut stripped).is_ok();
        if let Ok(normalized) = handwritten_json::normalize(&stripped);
        if let Ok(remote) = json5::from_str::<ModVersionMeta>(&normalized);
        then {
          if remote.version > local.version {
            (
              local.id,
              Ok(Some(remote))
            )
          } else {
            (
              local.id,
              Ok(None)
            )
          }
        } else {
          (
            local.id,
            Err(format!("Parse error. Payload:\n{}", remote))
          )
        }
      }
    }
  }


}

async fn send_request(url: String) -> Result<String, String>{
  reqwest::get(url)
    .await
    .map_err(|e| format!("{:?}", e))?
    .error_for_status()
    .map_err(|e| format!("{:?}", e))?
    .text()
    .await
    .map_err(|e| format!("{:?}", e))
}

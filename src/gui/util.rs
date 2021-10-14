use tinyfiledialogs as tfd;
use if_chain::if_chain;
use json_comments::strip_comments;
use std::io::Read;

use crate::gui::mod_list::ModVersionMeta;

pub fn error<T: AsRef<str>>(message: T) {
  tfd::message_box_ok("Error", sanitise(message).as_ref(), tfd::MessageBoxIcon::Error);
}

pub fn notif<T: AsRef<str>>(message: T) {
  tfd::message_box_ok("Message:", sanitise(message).as_ref(), tfd::MessageBoxIcon::Info);
}

pub fn query<T: AsRef<str>>(message: T) -> bool {
  match tfd::message_box_yes_no("Query:", sanitise(message).as_ref(), tfd::MessageBoxIcon::Question, tfd::YesNo::No) {
    tfd::YesNo::Yes => true,
    tfd::YesNo::No => false
  }
}

fn sanitise<T: AsRef<str>>(message: T) -> String {
  message.as_ref().replace("'", "`").replace("\"", "`")
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

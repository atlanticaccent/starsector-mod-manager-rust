use std::path::PathBuf;

use native_dialog::{MessageDialog, MessageType, FileDialog};

mod dialog;
pub use self::dialog::*;

impl DialogInterface for Dialog {
  fn error(message: String) {
    let mbox = move || {
      MessageDialog::new()
        .set_title("Alert:")
        .set_type(MessageType::Error)
        .set_text(&message)
        .show_alert()
        .map_err(|err| { err.to_string() })
    };
  
    // On windows we need to spawn a thread as the msg doesn't work otherwise
    #[cfg(target_os = "windows")]
    match std::thread::spawn(move || {
      mbox()
    }).join() {
      Ok(Ok(())) => Ok(()),
      Ok(Err(err)) => Err(err),
      Err(err) => Err(err).map_err(|err| format!("{:?}", err))
    }.unwrap();
    // unwrap() because if this goes to hell there's not really much we can do about it...
  
    #[cfg(not(target_os = "windows"))]
    mbox();
  }

  fn notif(message: String) {
    let mbox = move || {
      MessageDialog::new()
        .set_title("Alert:")
        .set_type(MessageType::Info)
        .set_text(&message)
        .show_alert()
        .map_err(|err| { err.to_string() })
    };
  
    // On windows we need to spawn a thread as the msg doesn't work otherwise
    #[cfg(target_os = "windows")]
    match std::thread::spawn(move || {
      mbox()
    }).join() {
      Ok(Ok(())) => Ok(()),
      Ok(Err(err)) => Err(err),
      Err(err) => Err(err).map_err(|err| format!("{:?}", err))
    }.unwrap();
    // unwrap() because if this goes to hell there's not really much we can do about it...
  
    #[cfg(not(target_os = "windows"))]
    mbox();
  }

  fn query(message: String) -> bool {
    let mbox = move || {
      MessageDialog::new()
      .set_type(MessageType::Warning)
      .set_text(&message)
      .show_confirm()
      .unwrap_or_default()
    };
  
    // On windows we need to spawn a thread as the msg doesn't work otherwise
    #[cfg(target_os = "windows")]
    let res = std::thread::spawn(move || {
      mbox()
    }).join().unwrap_or_default();
  
    #[cfg(not(target_os = "windows"))]
    let res = mbox();
  
    res
  }

  fn select_folder(_: &str, path: &str) -> Option<PathBuf> {
    FileDialog::new().set_location(path)
      .show_open_single_dir()
      .ok()
      .flatten()
  }

  fn select_file_dialog_multiple(_: &str, path: &str, filters: &[&str], description: &str) -> Option<Vec<PathBuf>> {
    FileDialog::new().set_location(path)
      .add_filter(description, filters)
      .show_open_multiple_file()
      .ok()
  }
}

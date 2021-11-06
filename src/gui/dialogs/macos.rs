use std::path::PathBuf;

use tinyfiledialogs as tfd;

mod dialog;
pub use self::dialog::*;

impl DialogInterface for Dialog {
  fn error(message: std::string::String) {
    tfd::message_box_ok("Error", &sanitise(message), tfd::MessageBoxIcon::Error);
  }

  fn notif(message: std::string::String) {
    tfd::message_box_ok("Message:", &sanitise(message), tfd::MessageBoxIcon::Info);
  }

  fn query(message: std::string::String) -> bool {
    match tfd::message_box_yes_no("Query:", &sanitise(message), tfd::MessageBoxIcon::Question, tfd::YesNo::No) {
      tfd::YesNo::Yes => true,
      tfd::YesNo::No => false
    }
  }
  
  fn select_folder(title: &str, path: &str) -> Option<PathBuf> {
    tfd::select_folder_dialog(title, path)
      .map(|p| PathBuf::from(p))
  }

  fn select_archives(path: &str) -> Option<Vec<PathBuf>> {
    Self::select_file_dialog_multiple("Select Archives", path, &["*.tar", "*.zip", "*.7z", "*.rar"], "Archive types")
  }

  fn select_file_dialog_multiple(title: &str, path: &str, filters: &[&str], description: &str) -> Option<Vec<PathBuf>> {
    tfd::open_file_dialog_multi(title, path, Some((filters, description)))
      .map(|paths| paths.into_iter().map(|path| PathBuf::from(path)).collect())
  }
}

fn sanitise(message: String) -> String {
  message.replace("'", "`").replace("\"", "`")
}

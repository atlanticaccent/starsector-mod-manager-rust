use std::path::PathBuf;

pub trait DialogInterface {
  fn error(message: String);

  fn notif(message: String);

  fn query(message: String) -> bool;

  fn select_folder(title: &str, path: &str) -> Option<PathBuf>;

  fn select_file_dialog_multiple(title: &str, path: &str, filter: &[&str], filter_label: &str) -> Option<Vec<PathBuf>>;
}

pub struct Dialog {}

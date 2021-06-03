use if_chain::if_chain;
use std::path::PathBuf;
use std::{
  fs::{copy, create_dir_all, read_dir},
  io,
};
use tokio::fs::{remove_dir_all, rename};

use crate::archive_handler;

#[derive(Debug, Clone)]
pub enum InstallError {
  DirectoryExists(PathBuf),
  DirectoryExistsFatal,
  DeleteError,
  NameError,
  NoModError,
  MoveError,
  UnsupportedArchive,
  ArchiveError,
}

pub async fn handle_archive(
  maybe_path: PathBuf,
  root_dir: PathBuf,
  retry: bool,
) -> Result<String, InstallError> {
  if_chain! {
    if let Some(path) = maybe_path.to_str();
    if let Some(_full_name) = maybe_path.file_name();
    if let Some(_file_name) = maybe_path.file_stem();
    if let Some(file_name) = _file_name.to_str();
    let mod_dir = root_dir.join("mods");
    let raw_temp_dest = mod_dir.join(format!("temp_{}", file_name));
    let raw_dest = mod_dir.join(_file_name);
    if let Some(temp_dest) = raw_temp_dest.to_str();
    then {
      if raw_dest.exists() {
        if retry {
          if remove_dir_all(&raw_dest).await.is_err() {
            return Err(InstallError::DirectoryExistsFatal)
          }
        } else {
          return Err(InstallError::DirectoryExists(maybe_path.clone()))
        }
      }

      match archive_handler::handle_archive(&path.to_owned(), &temp_dest.to_owned(), &file_name.to_owned()) {
        Ok(true) => {
          match find_nested_mod(&raw_temp_dest) {
            Ok(Some(mod_path)) => {
              if let Ok(_) = rename(mod_path, raw_dest).await {
                if raw_temp_dest.exists() {
                  if remove_dir_all(&raw_temp_dest).await.is_err() {
                    return Err(InstallError::DeleteError)
                  }
                }

                return Ok(format!("{:?}", _file_name))
              } else {
                return Err(InstallError::MoveError)
              }
            },
            _ => return Err(InstallError::NoModError)
          }
        },
        Ok(false) => return Err(InstallError::UnsupportedArchive),
        Err(_err) => return Err(InstallError::ArchiveError)
      }

    } else {
      return Err(InstallError::NameError)
    }
  }
}

fn find_nested_mod(dest: &PathBuf) -> Result<Option<PathBuf>, io::Error> {
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

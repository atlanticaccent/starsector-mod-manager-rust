use json_comments::strip_comments;
use if_chain::if_chain;
use std::path::PathBuf;
use std::{
  fs::{copy, create_dir_all, read_dir},
  io,
  io::Read
};
use tokio::fs::{remove_dir_all, rename};

use crate::archive_handler;
use crate::gui::mod_list::{ModEntry, ModVersionMeta};

#[derive(Debug, Clone)]
pub enum InstallError {
  DirectoryExists(PathBuf, bool),
  DeleteError(String),
  NameError,
  NoModError,
  MoveError,
  UnsupportedArchive,
  ArchiveError,
  CopyError,
  IDExists(PathBuf, PathBuf, Option<PathBuf>, String),
  NewModParseError
}

pub async fn handle_archive(
  source_path: PathBuf,
  root_dir: PathBuf,
  is_folder: bool,
  current_mods: Vec<String>
) -> Result<String, InstallError> {
  if_chain! {
    if let Some(path) = source_path.to_str();
    if let Some(_full_name) = source_path.file_name();
    if let Some(_file_name) = source_path.file_stem();
    if let Some(file_name) = _file_name.to_str();
    let mod_dir = root_dir.join("mods");
    let raw_temp_dest = mod_dir.join(format!("temp_{}", file_name));
    let raw_dest = mod_dir.join(_file_name);
    if let Some(temp_dest) = raw_temp_dest.to_str();
    then {
      if raw_dest.exists() {
        return Err(InstallError::DirectoryExists(source_path.clone(), is_folder))
      }

      if is_folder {
        if let Ok(Some(mod_path)) = find_nested_mod(&source_path) {
          if let Ok(new_mod_info) = ModEntry::from_file(mod_path.join("mod_info.json").clone()) {
            if let Some(id) = current_mods.iter().find(|id| **id == new_mod_info.id) {
              return Err(InstallError::IDExists(mod_path, raw_dest, None, id.clone()))
            }

            if let Err(_) = copy_dir_recursive(&raw_dest, &mod_path) {
              return Err(InstallError::CopyError)
            } else {
              if remove_dir_all(mod_path).await.is_err() {
                return Err(InstallError::DeleteError(format!("{:?}", _file_name)))
              } else {
                return Ok(format!("{:?}", _file_name))
              }
            }
          } else {
            return Err(InstallError::NoModError)
          }
        } else {
          Err(InstallError::NewModParseError)
        }
      } else {
        match archive_handler::handle_archive(&path.to_owned(), &temp_dest.to_owned(), &file_name.to_owned()) {
          Ok(true) => {
            match find_nested_mod(&raw_temp_dest) {
              Ok(Some(mod_path)) => {
                if let Ok(new_mod_info) = ModEntry::from_file(mod_path.join("mod_info.json").clone()) {
                  if let Some(id) = current_mods.iter().find(|id| **id == new_mod_info.id) {
                    return Err(InstallError::IDExists(mod_path, raw_dest, Some(raw_temp_dest), id.clone()))
                  }

                  if let Ok(_) = rename(mod_path, raw_dest).await {
                    if raw_temp_dest.exists() {
                      if remove_dir_all(&raw_temp_dest).await.is_err() {
                        return Err(InstallError::DeleteError(format!("{:?}", _file_name)))
                      }
                    }
    
                    return Ok(format!("{:?}", _file_name))
                  } else {
                    return Err(InstallError::MoveError)
                  }
                } else {
                  Err(InstallError::NewModParseError)
                }
              },
              _ => return Err(InstallError::NoModError)
            }
          },
          Ok(false) => return Err(InstallError::UnsupportedArchive),
          Err(_err) => return Err(InstallError::ArchiveError)
        }
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

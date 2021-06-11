use std::{fs, error::Error, path::Path};

macro_rules! dbg {
  ($($x:tt)*) => {
      {
          #[cfg(debug_assertions)]
          {
              std::dbg!($($x)*)
          }
          #[cfg(not(debug_assertions))]
          {
              ($($x)*)
          }
      }
  }
}

mod rar_patch {
  use std::{fmt, error::Error};

  pub struct UnrarErr {
    pub err: String,
  }

  impl UnrarErr {
    pub fn new() -> Self {
      UnrarErr {
        err: format!("Opaque Unrarr Error. Cannot extrapolate due to library limitation.")
      }
    }
  }
  
  impl fmt::Display for UnrarErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      return self.err.fmt(f);
    }
  }
  
  impl fmt::Debug for UnrarErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      return self.err.fmt(f);
    }
  }
  
  impl Error for UnrarErr {}
}

pub fn handle_archive(file: &String, dest: &String, file_name: &String) -> Result<bool, Box<dyn Error + Send>> {
  let kind = match infer::get_from_path(file) {
    Ok(res) => 
      match res {
        Some(kind) => kind,
        None => return Ok(false),
      },
    Err(err) => return Err(Box::new(err))
  };
  
  match kind.mime_type() {
    "application/vnd.rar" => {
      #[cfg(not(target_env="musl"))]
      return _rar_support(dest, file_name, file);
      
      #[cfg(target_env="musl")]
      compress_tools(file, dest)
    },
    "application/zip" => {
      match fs::File::open(file) {
        Ok(reader) => match zip::ZipArchive::new(reader) {
          Ok(mut archive) => {
            for i in 0..archive.len() {
              match archive.by_index(i) {
                Ok(mut file) => {
                  let outpath = match file.enclosed_name() {
                    Some(path) => Path::new(dest).join(path.to_owned()),
                    None => continue,
                  };
                  
                  if (&*file.name()).ends_with('/') {
                    dbg!("File {} extracted to \"{}\"", i, outpath.display());
                    if let Err(err) = fs::create_dir_all(&outpath) {
                      return Err(Box::new(err));
                    }
                  } else {
                    dbg!(
                      "File {} extracted to \"{}\" ({} bytes)",
                      i,
                      outpath.display(),
                      file.size()
                    );
                    if let Some(p) = outpath.parent() {
                      if !p.exists() {
                        if let Err(err) = fs::create_dir_all(&p) {
                          return Err(Box::new(err));
                        }
                      }
                    }
                    match fs::File::create(&outpath) {
                      Ok(mut outfile) => {
                        if let Err(err) = std::io::copy(&mut file, &mut outfile) {
                          return Err(Box::new(err));
                        }
                      }
                      Err(err) => return Err(Box::new(err))
                    }
                  }
                },
                Err(err) => return Err(Box::new(err))
              }
            }

            return Ok(true);
          }
          Err(err) => return Err(Box::new(err))
        },
        Err(err) => return Err(Box::new(err))
      }
    },
    "application/x-7z-compressed" => {
      #[cfg(target_family = "unix")]
      return compress_tools(file, dest);

      // null-op on windows
      #[cfg(not(target_family = "unix"))]
      Ok(false)
    },
    _ => {
      dbg!("is something else");
      return Ok(false);
    },
  }
}

fn compress_tools(file: &String, dest: &String) -> Result<bool, Box<dyn Error + Send>> {
  match fs::File::open(&file) {
    Ok(mut source) => {
      let dest = Path::new(dest);
      
      match compress_tools::uncompress_archive(&mut source, &dest, compress_tools::Ownership::Ignore) {
        Ok(()) => return Ok(true),
        Err(err) => return Err(Box::new(err)),
      }
    },
    Err(err) => Err(Box::new(err))
  }
}

// null-op on windows
#[cfg(not(target_family = "unix"))]
fn _7z_support(_: &String, _: &String) -> Result<bool, Box<dyn Error + Send>> {
  Ok(false)
}

fn _rar_support(dest: &String, file_name: &String, file: &String) -> Result<bool, Box<dyn Error + Send>> {
  let output_dir = Path::new(dest)
    .join(file_name)
    .to_string_lossy()
    .to_string();

  match unrar::Archive::new(file.clone()).extract_to(output_dir) {
    Ok(mut arch) => match arch.process() {
      Ok(_) => return Ok(true),
      Err(_) => return Err(Box::new(rar_patch::UnrarErr::new())),
    },
    Err(_) => return Err(Box::new(rar_patch::UnrarErr::new())),
  }
}

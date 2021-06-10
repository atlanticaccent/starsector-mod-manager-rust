use std::{fs, error::Error, path::Path};

#[cfg(not(target_family = "unix"))]
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

#[cfg(not(target_family = "unix"))]
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
                    println!("File {} extracted to \"{}\"", i, outpath.display());
                    if let Err(err) = fs::create_dir_all(&outpath) {
                      return Err(Box::new(err));
                    }
                  } else {
                    println!(
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
    _ => {
      println!("is something else");
      return Ok(false);
    },
  }
}

#[cfg(target_family = "unix")]
pub fn handle_archive(file: &String, dest: &String, _: &String) -> Result<bool, Box<dyn Error>> {
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

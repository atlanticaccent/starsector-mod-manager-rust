use std::{fs, fmt, error::Error, path::Path};

struct UnrarErr<T> {
  err: unrar::error::UnrarError<T>,
}

impl<T> fmt::Display for UnrarErr<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    return self.err.fmt(f);
  }
}

impl<T> fmt::Debug for UnrarErr<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    return self.err.fmt(f);
  }
}

impl<T> Error for UnrarErr<T> {}

pub fn handle_archive(file: &String) -> Result<bool, Box<dyn Error>>{
  let kind = match infer::get_from_path(&file) {
    Ok(res) => 
      match res {
        Some(kind) => kind,
        None => return Ok(false),
      },
    Err(err) => return Err(Box::new(err))
  };
  
  match kind.mime_type() {
    "application/vnd.rar" => {
      let output_dir = Path::new("./temp")
        .join(&file)
        .to_string_lossy()
        .to_string();

      match unrar::Archive::new(file.clone()).extract_to(output_dir) {
        Ok(mut arch) => match arch.process() {
          Ok(_) => return Ok(true),
          Err(err) => return Err(Box::new(UnrarErr { err })),
        },
        Err(err) => return Err(Box::new(UnrarErr { err })),
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
                    Some(path) => Path::new("./temp").join(path.to_owned()),
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
                      Ok(mut outfile) => if let Err(err) = std::io::copy(&mut file, &mut outfile) {
                        return Err(Box::new(err));
                      },
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
      let mut source = fs::File::open(&file).expect("Could not open file");
      let dest = Path::new("./tmp/dest");
      
      match compress_tools::uncompress_archive(&mut source, &dest, compress_tools::Ownership::Ignore) {
        Ok(()) => return Ok(true),
        Err(err) => return Err(Box::new(err)),
      }
    },
    _ => {
      println!("is something else");
      return Ok(false);
    },
  }
}

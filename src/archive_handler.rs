use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
  let args = std::env::args();
  let mut stderr = std::io::stderr();
  let file = args.skip(1).next().unwrap_or_else(|| {
    writeln!(&mut stderr, "Please pass an archive as argument!").unwrap();
    std::process::exit(0)
  });
  
  let kind = match infer::get_from_path(&file) {
    Ok(res) => 
      match res {
        Some(kind) => kind,
        None => return
      },
    Err(_) => {
      println!("Not a recognizable file type");
      return;
    }
  };
  
  match kind.mime_type() {
    "application/vnd.rar" => {
      let output_dir = Path::new("./temp")
        .join(&file)
        .to_string_lossy()
        .to_string();
      unrar::Archive::new(file.clone())
        .extract_to(output_dir)
        .unwrap()
        .process()
        .unwrap();
      println!("Done.");
    },
    "application/zip" => {
      let mut archive = zip::ZipArchive::new(fs::File::open(file).unwrap()).unwrap();
      
      for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = match file.enclosed_name() {
          Some(path) => Path::new("./temp").join(path.to_owned()),
          None => continue,
        };
        
        if (&*file.name()).ends_with('/') {
          println!("File {} extracted to \"{}\"", i, outpath.display());
          fs::create_dir_all(&outpath).unwrap();
        } else {
          println!(
            "File {} extracted to \"{}\" ({} bytes)",
            i,
            outpath.display(),
            file.size()
          );
          if let Some(p) = outpath.parent() {
            if !p.exists() {
              fs::create_dir_all(&p).unwrap();
            }
          }
          let mut outfile = fs::File::create(&outpath).unwrap();
          std::io::copy(&mut file, &mut outfile).unwrap();
        }
      }
    },
    "application/x-7z-compressed" => {
      let mut source = fs::File::open(&file).expect("Could not open file");
      let dest = Path::new("./tmp/dest");
      
      compress_tools::uncompress_archive(&mut source, &dest, compress_tools::Ownership::Ignore).expect("Could not decompress");
    },
    _ => println!("is something else"),
  }
}

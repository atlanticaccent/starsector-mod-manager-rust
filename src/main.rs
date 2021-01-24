use std::io::Write;

mod archive_handler;

fn main() {
  let args = std::env::args();
  let mut stderr = std::io::stderr();
  let file = args.skip(1).next().unwrap_or_else(|| {
    writeln!(&mut stderr, "Please pass an archive as argument!").unwrap();
    std::process::exit(0)
  });

  match archive_handler::handle_archive(&file) {
      Ok(res) => println!("Ran without exception. Completed? {}", res),
      Err(err) => println!("Encountered exception: {}", err),
  }
}
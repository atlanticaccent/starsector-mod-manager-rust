use std::{env, ops::Not, process};

fn main() {
  let mut cmd = process::Command::new(::std::env::var("CARGO").unwrap());
  let ref args = env::args().skip(1).collect::<Vec<_>>();
  // dbg!(args); cmd.arg("-vv"); /* Uncomment when debugging this hack */
  cmd.args(args);
  // if args...contains... "--target" "...macos..." ...
  if cfg!(target_os = "macos") {
    cmd.arg("--profile=fast-macos");
  } else if cfg!(target_os = "windows") {
    cmd.arg("--profile=fast");
    cmd.arg("--config");
    cmd.arg(r#"profile.fast.rustflags = ["-C", "embed-bitcode=no", "-C", "lto=off", "-Z", "share-generics=true", "-C", "target-feature=-crt-static"]"#);
  } else {
    cmd.arg("--profile=fast");
  }
  if cmd.status().ok().map_or(false, |it| it.success()).not() {
    process::exit(-1);
  }
}

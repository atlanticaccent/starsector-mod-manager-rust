use cfg_aliases::cfg_aliases;

fn main() {
  cfg_aliases! {
    linux: { any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "netbsd", target_os = "openbsd") },
    mac: { target_os = "macos" }
  }
}

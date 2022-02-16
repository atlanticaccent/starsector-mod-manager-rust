use self_update::{backends::github::Update, errors::Error as SelfUpdateError, cargo_crate_version};

pub fn support_self_update() -> bool {
  #[cfg(target_os = "macos")]
  return false;
  #[cfg(not(target_os = "macos"))]
  true
}

pub fn self_update() -> Result<(), SelfUpdateError> {
  Update::configure()
    .repo_owner("atlanticaccent")
    .repo_name("starsector-mod-manager-rust")
    .current_version(cargo_crate_version!())
    .target({
      #[cfg(target_os = "windows")]
      let bin = "starsector_mod_manager.exe";
      #[cfg(all(target_os = "linux", target_feature = "crt-static"))]
      let bin = "starsector_mod_manager_linux_dynamic";
      #[cfg(all(target_os = "linux", not(target_feature = "crt-static")))]
      let bin = "starsector_mod_manager_linux_static";

      bin
    })
    .bin_name({
      #[cfg(target_os = "windows")]
      let bin = "tmp_starsector_mod_manager.exe";
      #[cfg(all(target_os = "linux", target_feature = "crt-static"))]
      let bin = "tmp_starsector_mod_manager_linux_dynamic";
      #[cfg(all(target_os = "linux", not(target_feature = "crt-static")))]
      let bin = "tmp_starsector_mod_manager_linux_static";

      bin
    })
    .no_confirm(true)
    .build()?
    .update()?;

  Ok(())
}

pub fn open_in_browser() {
  if let Err(_) = opener::open("https://github.com/atlanticaccent/starsector-mod-manager-rust/releases") {
    eprintln!("Failed to open GitHub");
  }
}

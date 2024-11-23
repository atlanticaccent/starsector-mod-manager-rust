use derive_more::derive::{Deref, From};
use self_update::{
  backends::github,
  cargo_crate_version,
  update::{Release as ReleaseInternal, ReleaseUpdate},
  version,
};
use tokio::sync::oneshot;
use types::CloneTx;

#[derive(Debug, Clone, From, Deref)]
#[repr(transparent)]
pub struct Release(ReleaseInternal);

impl PartialEq for Release {
  fn eq(&self, other: &Self) -> bool {
    self.name == other.name && self.version == other.version && self.date == other.version
  }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
  Ready(Release, CloneTx),
  Completed,
  CheckFailed(String),
  InstallFailed(String),
}

// const SUPPORT_SELF_UPDATE: bool = cfg!(not(target_os = "macos"));
const TARGET: &str = if cfg!(target_os = "windows") {
  "MOSS.exe"
} else if cfg!(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
)) {
  "MOSS.AppImage"
} else {
  "MOSS.app"
};
const CURRENT_VERSION: &str = cargo_crate_version!();

pub fn check_for_update(callback: impl Fn(Status) + Send + Sync + 'static) {
  tokio::task::spawn_blocking(move || check_for_update_blocking(&callback, &callback));
}

pub fn check_for_update_opts(
  update_callback: impl FnOnce(Status) + Send + 'static,
  finish_callback: impl FnOnce(Status) + Send + 'static,
) {
  tokio::task::spawn_blocking(|| check_for_update_blocking(update_callback, finish_callback));
}

fn check_for_update_blocking(
  update_callback: impl FnOnce(Status),
  finish_callback: impl FnOnce(Status),
) {
  let updater = match get_updater() {
    Ok(updater) => updater,
    Err(err) => return update_callback(Status::CheckFailed(err.to_string())),
  };

  let release = match updater.get_latest_release() {
    Ok(release) => Release::from(release),
    Err(err) => return update_callback(Status::CheckFailed(err.to_string())),
  };

  match version::bump_is_greater(CURRENT_VERSION, &release.version) {
    Ok(true) => println!("Update found"),
    Ok(false) => return println!("Up to date"),
    Err(err) => return update_callback(Status::CheckFailed(err.to_string())),
  };

  let (tx, rx) = oneshot::channel();
  update_callback(Status::Ready(release.clone(), CloneTx::new(tx)));

  // There should only ever be one thread blocked here
  // should wake and return if another ever does
  if rx.blocking_recv().unwrap_or_default() {
    let result = if let Err(err) = update(updater, release) {
      Status::InstallFailed(err.to_string())
    } else {
      Status::Completed
    };

    finish_callback(result);
  };
}

fn get_updater() -> anyhow::Result<Box<dyn ReleaseUpdate>> {
  let updater = github::Update::configure()
    .repo_owner("atlanticaccent")
    .repo_name({
      assert!(CURRENT_VERSION != "0.8.0");
      "test"
    })
    .current_version(CURRENT_VERSION)
    .target(TARGET)
    .bin_name(TARGET)
    .show_output(false)
    .no_confirm(true)
    .build()?;

  Ok(updater)
}

// #[cfg(not(windows))]
#[cfg(windows)]
fn update(updater: Box<dyn ReleaseUpdate>, _: Release) -> anyhow::Result<()> {
  updater.update()?;

  Ok(())
}

#[cfg(not(windows))]
// #[cfg(windows)]
fn update(_: Box<dyn ReleaseUpdate>, release: Release) -> anyhow::Result<()> {
  use std::path::PathBuf;

  use anyhow::Context;
  use cargo_packager_updater::{Config, Update, UpdateFormat};
  use self_update::Download;

  const FORMAT: UpdateFormat = if cfg!(target_os = "macos") {
    UpdateFormat::App
  } else if cfg!(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
  )) {
    UpdateFormat::AppImage
  } else {
    UpdateFormat::Wix
  };

  fn default_update() -> Update {
    Update {
      config: Config::default(),
      body: None,
      current_version: String::new(),
      version: String::new(),
      date: None,
      target: String::new(),
      extract_path: PathBuf::new(),
      download_url: "fa:ke".parse().unwrap(),
      signature: String::new(),
      timeout: None,
      headers: Default::default(),
      format: FORMAT,
    }
  }

  let asset = release
    .asset_for(TARGET, None)
    .with_context(|| format!("Release missing update for {}", self_update::get_target()))?;

  let mut buf = Vec::new();
  Download::from_url(&asset.download_url).download_to(&mut buf)?;

  default_update().install(buf)?;

  Ok(())
}

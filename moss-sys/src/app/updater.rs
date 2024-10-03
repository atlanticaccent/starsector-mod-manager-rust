use std::{
  ops::Deref,
  sync::{Arc, Mutex},
};

use druid::{Data, ExtEventSink};
use self_update::{
  backends::github,
  cargo_crate_version,
  update::{Release as ReleaseInternal, ReleaseUpdate},
  version,
};
use tokio::sync::oneshot;
use webview_shared::ExtEventSinkExt;

use crate::{
  app::overlays::{Popup, Status},
  d_println,
};

#[derive(Debug, Clone, Data, derive_more::From)]
#[repr(transparent)]
pub struct Release(#[data(same_fn = "release_eq")] ReleaseInternal);

fn release_eq(left: &ReleaseInternal, right: &ReleaseInternal) -> bool {
  left.name.same(&right.name) && left.version.same(&right.version) && left.date.same(&right.version)
}

impl Deref for Release {
  type Target = ReleaseInternal;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

// const SUPPORT_SELF_UPDATE: bool = cfg!(not(target_os = "macos"));
const TARGET: &str = if cfg!(target_os = "windows") {
  "moss.exe"
} else {
  "moss_linux"
};
const CURRENT_VERSION: &str = cargo_crate_version!();

#[derive(Debug, Clone, Data)]
pub struct CopyTx(Arc<Mutex<Option<oneshot::Sender<bool>>>>);

impl CopyTx {
  pub fn new(tx: oneshot::Sender<bool>) -> Self {
    Self(Arc::new(Mutex::new(Some(tx))))
  }

  pub fn send(&self, val: bool) {
    let Ok(mut guard) = self.0.lock() else {
      return;
    };

    if let Some(sender) = guard.take() {
      let _ = sender.send(val);
    }
  }
}

pub async fn check_for_update(ext_ctx: ExtEventSink) {
  tokio::task::spawn_blocking(move || {
    d_println!("Starting update check");
    let send_self_update_popup = |status: Status| {
      ext_ctx
        .submit_command_global(Popup::OPEN_POPUP, Popup::SelfUpdate(status))
        .expect("Submit cmd");
    };

    let updater = match get_updater() {
      Ok(updater) => updater,
      Err(err) => return send_self_update_popup(Status::CheckFailed(err.to_string())),
    };

    let release = match updater.get_latest_release() {
      Ok(release) => Release::from(release),
      Err(err) => return send_self_update_popup(Status::CheckFailed(err.to_string())),
    };

    match version::bump_is_greater(CURRENT_VERSION, &release.version) {
      Ok(true) => d_println!("Update found"),
      Ok(false) => return d_println!("Up to date"),
      Err(err) => return send_self_update_popup(Status::CheckFailed(err.to_string())),
    };

    #[cfg(not(target_os = "macos"))]
    {
      let (tx, rx) = oneshot::channel();
      send_self_update_popup(Status::Ready(release, CopyTx::new(tx)));

      if rx.blocking_recv().unwrap_or_default() {
        let result = if updater.update().is_ok() {
          Status::Completed
        } else {
          Status::InstallFailed
        };

        send_self_update_popup(result);
      };
    }
    #[cfg(target_os = "macos")]
    {
      send_self_update_popup(Status::Ready(release));
    }
  });
}

pub fn get_updater() -> anyhow::Result<Box<dyn ReleaseUpdate>> {
  let updater = github::Update::configure()
    .repo_owner("atlanticaccent")
    .repo_name("test")
    .current_version(CURRENT_VERSION)
    .target(TARGET)
    .bin_path_in_archive(TARGET)
    .bin_name("moss")
    .show_output(false)
    .no_confirm(true)
    .build()?;

  Ok(updater)
}

#[cfg(not(target_os = "windows"))]
pub fn alternative_updater(url: &str) -> anyhow::Result<()> {
  use std::path::PathBuf;

  use cargo_packager_updater as alt_updater;
  use self_update::Download;

  const FORMAT: alt_updater::UpdateFormat = if cfg!(target_os = "macos") {
    alt_updater::UpdateFormat::App
  } else if cfg!(target_os = "linux") {
    alt_updater::UpdateFormat::AppImage
  } else {
    alt_updater::UpdateFormat::Wix
  };

  fn default_update() -> alt_updater::Update {
    alt_updater::Update {
      config: alt_updater::Config {
        endpoints: Vec::new(),
        pubkey: String::new(),
        windows: None,
      },
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

  let mut buf = Vec::new();

  Download::from_url(url).download_to(&mut buf)?;

  default_update().install(buf)?;

  Ok(())
}

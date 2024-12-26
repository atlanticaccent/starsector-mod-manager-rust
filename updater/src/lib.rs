#![feature(error_reporter)]

use std::sync::LazyLock;

use anyhow::Context;
use derive_more::derive::{Deref, From};
use self_update::{
  backends::github,
  cargo_crate_version,
  update::{Release as ReleaseInternal, ReleaseAsset, ReleaseUpdate},
  version,
};
use tokio::sync::oneshot;
use types::CloneTx;
use web_client::WebClient;

const REPO_NAME: LazyLock<&'static str> = LazyLock::new(|| {
  if CURRENT_VERSION < "0.8.0" {
    "test"
  } else {
    "starsector-mod-manager-rust"
  }
});

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
  tokio::runtime::Handle::current()
    .spawn(async move { check_for_update_async(&callback, &callback).await });
}

async fn check_for_update_async(
  update_callback: impl FnOnce(Status),
  finish_callback: impl FnOnce(Status),
) {
  async fn check_for_update_impl() -> anyhow::Result<Option<ReleaseInternal>> {
    let api_url = format!(
      "{}/repos/{}/{}/releases/latest",
      "https://api.github.com", "atlanticaccent", *REPO_NAME,
    );
    let resp = WebClient::from_reqwest_builder(
      WebClient::default_reqwest_builder().danger_accept_invalid_certs(true),
      WebClient::default_retry_policy(),
    )
    .build()
    .get(&api_url)
    .send()
    .await?;
    if !resp.status().is_success() {
      anyhow::bail!(
        "api request failed with status: {:?} - for: {:?}",
        resp.status(),
        api_url
      )
    }

    let release_raw = resp.json::<serde_json::Value>().await?;
    let tag = release_raw["tag_name"]
      .as_str()
      .context("Release missing `tag_name`")?;
    let date = release_raw["created_at"]
      .as_str()
      .context("Release missing `created_at`")?;
    let name = release_raw["name"].as_str().unwrap_or(tag);
    let assets = release_raw["assets"]
      .as_array()
      .context("No assets found")?;
    let body = release_raw["body"].as_str().map(String::from);
    let assets = assets
      .iter()
      .map(|asset| {
        let download_url = asset["url"].as_str().context("Asset missing `url`")?;
        let name = asset["name"].as_str().context("Asset missing `name`")?;
        Ok(ReleaseAsset {
          download_url: download_url.to_owned(),
          name: name.to_owned(),
        })
      })
      .collect::<anyhow::Result<Vec<ReleaseAsset>>>()?;

    let release_internal = ReleaseInternal {
      name: name.to_owned(),
      version: tag.trim_start_matches('v').to_owned(),
      date: date.to_owned(),
      body,
      assets,
    };

    if version::bump_is_greater(CURRENT_VERSION, &release_internal.version)? {
      println!("Update found");
      Ok(Some(release_internal))
    } else {
      println!("Up to date");
      Ok(None)
    }
  }

  let release = match check_for_update_impl().await {
    Ok(Some(release_internal)) => Release(release_internal),
    Ok(None) => return,
    Err(err) => {
      let reporter = std::error::Report::new(dbg!(err.root_cause())).pretty(true);

      return update_callback(Status::CheckFailed(dbg!(reporter).to_string()));
    }
  };

  let (tx, rx) = oneshot::channel();
  update_callback(Status::Ready(release.clone(), CloneTx::new(tx)));

  // There should only ever be one thread blocked here
  // should wake and return if another ever does
  if rx.await.unwrap_or_default() {
    let result = if let Err(err) =
      tokio::task::block_in_place(|| get_updater().and_then(|updater| update(updater, release)))
    {
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
    .repo_name(&REPO_NAME)
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

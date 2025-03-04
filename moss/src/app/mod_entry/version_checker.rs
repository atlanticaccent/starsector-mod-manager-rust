use std::{borrow::Borrow, fmt::Display, hash::Hash, sync::Arc};

use common::theme_keys::{
  BLUE_KEY, GREEN_KEY, ON_BLUE_KEY, ON_GREEN_KEY, ON_ORANGE_KEY, ON_RED_KEY, ON_YELLOW_KEY,
  ORANGE_KEY, RED_KEY, YELLOW_KEY,
};
use druid::{Color, Data, KeyOrValue, Lens};
use fake::Dummy;
use serde::Deserialize;
use serde_aux::prelude::deserialize_string_from_number;

use crate::app::mod_entry::{ModEntry, VersionComplex};

pub(crate) type AsyncModVersionMetaRes = Result<ModVersionMeta, Arc<anyhow::Error>>;

#[derive(Debug, Clone, Data, Lens, Hash, derive_more::Deref)]
pub struct VersionChecker {
  #[deref]
  pub local: ModVersionMeta,
  remote: Option<ModVersionMeta>,
  pub update_status: UpdateStatus,
}

impl VersionChecker {
  pub fn new(local: ModVersionMeta) -> Self {
    Self {
      local,
      remote: None,
      update_status: UpdateStatus::Error,
    }
  }

  pub fn update_remote(&mut self, remote: Option<ModVersionMeta>) {
    self.remote = remote;
    self.update_status = UpdateStatus::from((&self.local, &self.remote))
  }

  pub fn get_remote(&self) -> Option<&ModVersionMeta> {
    self.remote.as_ref()
  }

  pub fn get_direct_download_url(&self) -> Option<&str> {
    self
      .get_remote()
      .and_then(|remote| remote.direct_download_url.as_deref())
  }
}

impl PartialEq for VersionChecker {
  fn eq(&self, other: &Self) -> bool {
    self.local == other.local
      && self.remote == other.remote
      && self.update_status == other.update_status
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Data, Hash, Dummy)]
pub enum UpdateStatus {
  Error,
  UpToDate,
  Discrepancy(VersionComplex),
  Patch(VersionComplex),
  Minor(VersionComplex),
  Major(VersionComplex),
}

impl Display for UpdateStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    match self {
      UpdateStatus::Major(remote) => write!(f, "Major update available: {remote}"),
      UpdateStatus::Minor(remote) => write!(f, "Minor update available: {remote}"),
      UpdateStatus::Patch(remote) => write!(f, "Patch available: {remote}"),
      UpdateStatus::UpToDate => write!(f, "Up to date"),
      UpdateStatus::Error => write!(f, "Error"),
      UpdateStatus::Discrepancy(_) => write!(f, "Discrepancy"),
    }
  }
}

impl<VL: Borrow<VersionComplex>, VR: Borrow<VersionComplex>> From<(VL, Option<VR>)>
  for UpdateStatus
{
  fn from((local, remote): (VL, Option<VR>)) -> Self {
    if let Some(remote) = remote {
      let local = local.borrow();
      let remote = remote.borrow().clone();

      if remote == *local {
        UpdateStatus::UpToDate
      } else if remote < *local {
        UpdateStatus::Discrepancy(remote)
      } else if remote.major - local.major > 0 {
        UpdateStatus::Major(remote)
      } else if remote.minor - local.minor > 0 {
        UpdateStatus::Minor(remote)
      } else {
        UpdateStatus::Patch(remote)
      }
    } else {
      UpdateStatus::Error
    }
  }
}

impl From<(&ModVersionMeta, &Option<ModVersionMeta>)> for UpdateStatus {
  fn from((local, remote): (&ModVersionMeta, &Option<ModVersionMeta>)) -> Self {
    (&local.version, remote.as_ref().map(|r| &r.version)).into()
  }
}

impl From<&UpdateStatus> for KeyOrValue<Color> {
  fn from(status: &UpdateStatus) -> Self {
    match status {
      UpdateStatus::Major(_) => ORANGE_KEY.into(),
      UpdateStatus::Minor(_) => YELLOW_KEY.into(),
      UpdateStatus::Patch(_) => BLUE_KEY.into(),
      UpdateStatus::Discrepancy(_) => Color::from_hex_str("#810181").unwrap().into(),
      UpdateStatus::Error => RED_KEY.into(),
      UpdateStatus::UpToDate => GREEN_KEY.into(),
    }
  }
}

impl UpdateStatus {
  pub fn as_text_colour(&self) -> KeyOrValue<Color> {
    match self {
      UpdateStatus::Major(_) => ON_ORANGE_KEY.into(),
      UpdateStatus::Minor(_) => ON_YELLOW_KEY.into(),
      UpdateStatus::Patch(_) => ON_BLUE_KEY.into(),
      UpdateStatus::Discrepancy(_) => Color::from_hex_str("#ffd6f7").unwrap().into(),
      UpdateStatus::Error => ON_RED_KEY.into(),
      UpdateStatus::UpToDate => ON_GREEN_KEY.into(),
    }
  }
}

#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Debug, Clone, Deserialize, Eq, Data, Lens, Hash, Dummy)]
pub struct ModVersionMeta {
  #[serde(alias = "masterVersionFile")]
  pub remote_url: String,
  #[serde(alias = "directDownloadURL")]
  #[serde(default)]
  pub direct_download_url: Option<String>,
  #[serde(alias = "modName")]
  pub id: String,
  #[serde(alias = "modThreadId")]
  #[serde(deserialize_with = "deserialize_string_from_number")]
  #[serde(default)]
  pub fractal_id: String,
  #[serde(alias = "modNexusId")]
  #[serde(deserialize_with = "deserialize_string_from_number")]
  #[serde(default)]
  pub nexus_id: String,
  #[serde(alias = "modVersion")]
  pub version: VersionComplex,
}

impl installer::EntryUpdate for ModVersionMeta {
  type Entry = ModEntry;

  fn url(&self) -> String {
    self.direct_download_url.clone().unwrap()
  }

  fn matches(&self, entry: &ModEntry) -> bool {
    entry
      .version_checker
      .as_ref()
      .map(|vc| vc.local.version == self.version)
      .unwrap_or_default()
  }
}

impl PartialEq for ModVersionMeta {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.version == other.version
  }
}

impl PartialOrd for ModVersionMeta {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.version.cmp(&other.version))
  }
}

impl Ord for ModVersionMeta {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.partial_cmp(other).unwrap()
  }
}

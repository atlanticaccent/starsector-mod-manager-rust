use druid::Data;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::SETTINGS_VERSION;

pub(super) const SETTINGS_VERSION_TAG: VersionTag = VersionTag(Some(SETTINGS_VERSION));

#[derive(Debug, Clone, PartialEq, Data)]
pub(super) struct VersionTag(Option<u8>);

impl Serialize for VersionTag {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    Some(SETTINGS_VERSION).serialize(serializer)
  }
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum TagError {
  #[error("Incompatible config version: {0} | Current: {SETTINGS_VERSION}")]
  Incompatible(u8),
}

impl<'de> Deserialize<'de> for VersionTag {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let version = <Option<u8>>::deserialize(deserializer)?.unwrap_or(0);

    if version == SETTINGS_VERSION {
      Ok(Self(Some(version)))
    } else {
      Err(serde::de::Error::custom(TagError::Incompatible(version)))
    }
  }
}

impl Default for VersionTag {
  #[inline(always)]
  fn default() -> Self {
    SETTINGS_VERSION_TAG
  }
}

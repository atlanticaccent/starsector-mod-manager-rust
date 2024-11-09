use std::{
  borrow::Cow,
  path::{Path, PathBuf},
  sync::Arc,
};

use tempfile::TempDir;

#[derive(Debug, Clone)]
pub enum HybridPath {
  PathBuf(PathBuf),
  Temp(Arc<TempDir>, String, Option<PathBuf>),
}

impl HybridPath {
  pub fn get_path_copy(&self) -> PathBuf {
    match self {
      HybridPath::PathBuf(ref path) | HybridPath::Temp(_, _, Some(ref path)) => path.clone(),
      HybridPath::Temp(ref arc, _, None) => arc.path().to_path_buf(),
    }
  }

  pub fn with_path(mut self, path: &Path) -> Self {
    match &mut self {
      HybridPath::PathBuf(inner) => path.clone_into(inner),
      HybridPath::Temp(_, _, path_opt) => {
        path_opt.replace(path.to_owned());
      }
    };

    self
  }

  pub fn source(&self) -> Cow<str> {
    match self {
      HybridPath::PathBuf(path) => path.to_string_lossy(),
      HybridPath::Temp(_, source, _) => source.into(),
    }
  }
}

impl PartialEq for HybridPath {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::PathBuf(l0), Self::PathBuf(r0)) => l0 == r0,
      (Self::Temp(l0, l1, l2), Self::Temp(r0, r1, r2)) => {
        l0.path() == r0.path() && l1 == r1 && l2 == r2
      }
      _ => false,
    }
  }
}

impl From<PathBuf> for HybridPath {
  fn from(value: PathBuf) -> Self {
    Self::PathBuf(value)
  }
}

use std::{
  error::Error,
  future::Future,
  path::{Path, PathBuf},
};

use crate::{HybridPath, StringOrPath};

pub trait Entry: for<'a> TryFrom<&'a Path> + Send + 'static {
  type Id: Into<StringOrPath>;
  type Error: std::error::Error + Send + Sync;

  fn id(&self) -> Self::Id;

  fn destination_folder(&self, parent: &Path) -> PathBuf;

  fn enrich(
    &mut self,
    path: PathBuf,
  ) -> impl Future<Output = Result<(), <Self as Entry>::Error>> + Send;
}

pub trait InstallerDelegate: Clone + Send + 'static {
  type Entry: Entry;

  fn error_handler(&self, error: &dyn Error);

  fn multiple_handler(&self, folder: HybridPath, found: Vec<Self::Entry>);

  fn overwrite_handler(&self, found: StringOrPath, folder: HybridPath, entry: Self::Entry);

  fn completed_handler(&self, entry: Self::Entry);

  fn check_conflict(&self, entry: &Self::Entry) -> impl Future<Output = bool> + Send + 'static;
}

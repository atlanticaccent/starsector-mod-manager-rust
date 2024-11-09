use std::{
  error::Error,
  fmt::Debug,
  future::Future,
  path::{Path, PathBuf},
};

use futures_util::TryFutureExt as _;

pub use crate::installer::InstallerExt;
use crate::{HybridPath, InstallError, StringOrPath};

pub trait Entry: Debug + Sized + Send + 'static {
  type Id: Into<StringOrPath>;
  type EnrichmentError: Error + Send + Sync;
  type ParseError: Error + Send + Sync;

  fn id(&self) -> Self::Id;

  fn parse(
    path: impl AsRef<Path> + Send,
  ) -> impl Future<Output = Result<Self, <Self as Entry>::ParseError>> + Send;

  fn destination_folder(&self, parent: &Path) -> PathBuf;

  fn enrich(
    &mut self,
    path: PathBuf,
  ) -> impl Future<Output = Result<(), <Self as Entry>::EnrichmentError>> + Send;
}

pub(crate) trait EntryExt: Entry {
  fn parse_ext(
    path: impl AsRef<Path> + Send,
  ) -> impl Future<Output = Result<Self, InstallError<Self>>> + Send {
    Self::parse(path).map_err(InstallError::EntryParsingError)
  }
}

impl<T: Entry> EntryExt for T {}

pub trait EntryUpdate: Send {
  type Entry: Entry;

  fn url(&self) -> String;

  fn matches(&self, entry: &Self::Entry) -> bool;
}

pub trait InstallerDelegate: Clone + Send + Sync {
  type Entry: Entry;
  type EntryUpdate: EntryUpdate<Entry = Self::Entry>;

  fn error_handler<E: Error + Send + Sync + 'static>(&self, error: E);

  fn multiple_handler(&self, folder: HybridPath, found: Vec<Self::Entry>);

  fn overwrite_handler(&self, found: StringOrPath, folder: HybridPath, entry: Self::Entry);

  fn completed_handler(&self, entry: Self::Entry);

  fn check_conflict(&self, entry: &Self::Entry) -> impl Future<Output = bool> + Send + 'static;
}

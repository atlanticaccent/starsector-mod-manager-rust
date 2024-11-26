use std::{future::Future, path::PathBuf, sync::Arc};

use common::ExtEventSinkExt;
use druid::{ExtEventSink, Selector, SingleUse};
use installer::{Entry, HybridPath, InstallerDelegate, InstallerExt, Request};
use tokio::sync::oneshot::Sender;

use super::{mod_entry::ModVersionMeta, overlays::Popup};
use crate::{app::mod_entry::ModEntry, bang};

pub const INSTALL: Selector<SingleUse<InstallMessage>> = Selector::new("install.message");
pub const DOWNLOAD_STARTED: Selector<(i64, String)> = Selector::new("install.download.started");
pub const DOWNLOAD_PROGRESS: Selector<Vec<(i64, String, f64)>> =
  Selector::new("install.download.progress");

#[derive(Debug)]
pub enum InstallMessage {
  /// Multiple mods found in single installation source
  FoundMultiple(Vec<PathBuf>, HybridPath),
  /// Check mod already installed
  CheckConflict(String, Sender<bool>),
  /// Installation succeeded
  Success(Box<ModEntry>),
  /// Unrecoverable install error
  Error(String, Arc<dyn std::error::Error + Send + Sync>),
}

#[derive(Clone)]
pub struct Installer {
  ext_ctx: ExtEventSink,
}

impl Installer {
  pub fn new(ext_ctx: ExtEventSink) -> Self {
    Self { ext_ctx }
  }

  pub fn install(
    &self,
    request: Request<ModEntry, ModVersionMeta>,
  ) -> impl Future<Output = ()> + Send {
    let installer = self.clone();
    async move {
      <Self as InstallerExt>::install(&installer, request).await;
    }
  }
}

pub trait AsyncError = std::error::Error + Send + Sync + 'static;

impl InstallerDelegate for Installer {
  type Entry = ModEntry;
  type EntryUpdate = ModVersionMeta;

  fn error_handler<E: AsyncError>(&self, error: E) {
    let _ = self
      .ext_ctx
      .submit_command_global(
        INSTALL,
        SingleUse::new(InstallMessage::Error(String::new(), Arc::new(error))),
      )
      .inspect_err(|err| bang!(err));
  }

  fn multiple_handler(&self, folder: HybridPath, found: Vec<ModEntry>) {
    let _ = self
      .ext_ctx
      .submit_command_global(Popup::OPEN_POPUP, Popup::found_multiple(folder, found))
      .inspect_err(|err| bang!(err));
  }

  fn overwrite_handler(&self, found: installer::StringOrPath, folder: HybridPath, entry: ModEntry) {
    let _ = self
      .ext_ctx
      .submit_command_global(Popup::QUEUE_POPUP, Popup::overwrite(found, folder, entry))
      .inspect_err(|err| bang!(err));
  }

  fn completed_handler(&self, entry: ModEntry) {
    let _ = self
      .ext_ctx
      .submit_command_global(
        INSTALL,
        SingleUse::new(InstallMessage::Success(Box::new(entry))),
      )
      .inspect_err(|err| bang!(err));
  }

  fn check_conflict(&self, entry: &ModEntry) -> impl Future<Output = bool> + Send + 'static {
    let ext_ctx = self.ext_ctx.clone();
    let id = entry.id();
    async move {
      let (tx, rx) = tokio::sync::oneshot::channel();

      if ext_ctx
        .submit_command_global(
          INSTALL,
          SingleUse::new(InstallMessage::CheckConflict(id, tx)),
        )
        .inspect_err(|err| bang!(err))
        .is_err()
      {
        return false;
      }

      rx.await.unwrap_or_default()
    }
  }
}

use std::{future::Future, path::PathBuf, sync::Arc};

use druid::{ExtEventSink, Selector, SingleUse};
use moss_lib::installer::{Entry, HybridPath, InstallerDelegate, InstallerExt, Request};
use webview_shared::ExtEventSinkExt;

use super::{mod_entry::ModVersionMeta, overlays::Popup};
use crate::{app::mod_entry::ModEntry, bang};

pub const INSTALL: Selector<ChannelMessage> = Selector::new("install.message");
pub const DOWNLOAD_STARTED: Selector<(i64, String)> = Selector::new("install.download.started");
pub const DOWNLOAD_PROGRESS: Selector<Vec<(i64, String, f64)>> =
  Selector::new("install.download.progress");
pub const INSTALL_FOUND_MULTIPLE: Selector<SingleUse<(Vec<PathBuf>, HybridPath)>> =
  Selector::new("install.found_multiple.install_all");

#[derive(Debug, Clone)]
pub enum ChannelMessage {
  /// New mod entry
  Success(Box<ModEntry>),
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
    <Self as InstallerExt>::install(self.clone(), request)
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
        ChannelMessage::Error(String::new(), Arc::new(error)),
      )
      .inspect_err(|err| bang!(err));
  }

  fn multiple_handler(&self, folder: HybridPath, found: Vec<ModEntry>) {
    let _ = self
      .ext_ctx
      .submit_command_global(Popup::OPEN_POPUP, Popup::found_multiple(folder, found))
      .inspect_err(|err| bang!(err));
  }

  fn overwrite_handler(
    &self,
    found: moss_lib::installer::StringOrPath,
    folder: HybridPath,
    entry: ModEntry,
  ) {
    let _ = self
      .ext_ctx
      .submit_command_global(Popup::QUEUE_POPUP, Popup::overwrite(found, folder, entry))
      .inspect_err(|err| bang!(err));
  }

  fn completed_handler(&self, entry: ModEntry) {
    let _ = self
      .ext_ctx
      .submit_command_global(INSTALL, ChannelMessage::Success(Box::new(entry)))
      .inspect_err(|err| bang!(err));
  }

  fn check_conflict(&self, entry: &ModEntry) -> impl Future<Output = bool> + Send + 'static {
    let ext_ctx = self.ext_ctx.clone();
    let id = entry.id();
    async move {
      let _ = ext_ctx;
      let _ = id;
      false
    }
  }
}

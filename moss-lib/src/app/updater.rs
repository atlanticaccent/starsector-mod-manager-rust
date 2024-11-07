use druid::ExtEventSink;
use updater::Status;
use common::ExtEventSinkExt;

use crate::app::overlays::Popup;

pub fn get_update_status_handler(ext_ctx: ExtEventSink) -> impl Fn(Status) {
  move |status| {
    ext_ctx
      .submit_command_global(Popup::OPEN_POPUP, Popup::SelfUpdate(status))
      .expect("Submit cmd");
  }
}

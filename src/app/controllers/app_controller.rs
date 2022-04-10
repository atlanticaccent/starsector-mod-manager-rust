use std::process;

use druid::{widget::Controller, Env, Event, EventCtx, Target, Widget, commands};
use rfd::FileDialog;
use self_update::version::bump_is_greater;

use crate::app::{
  modal::Modal,
  settings::{self, Settings, SettingsCommand},
  updater::{open_in_browser, self_update, support_self_update},
  App, TAG, mod_list::ModList,
};

pub struct AppController;

impl<W: Widget<App>> Controller<App, W> for AppController {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut App, env: &Env) {
    if let Event::Command(cmd) = event {
      if let Some(settings::SettingsCommand::SelectInstallDir) = cmd.get(Settings::SELECTOR) {
        let ext_ctx = ctx.get_external_handle();
        ctx.set_disabled(true);
        data.runtime.spawn_blocking(move || {
          let res = FileDialog::new().pick_folder();

          if let Some(handle) = res {
            ext_ctx.submit_command(
              Settings::SELECTOR,
              SettingsCommand::UpdateInstallDir(handle),
              Target::Auto,
            )
          } else {
            ext_ctx.submit_command(App::ENABLE, (), Target::Auto)
          }
        });
      } else if let Some(()) = cmd.get(App::DUMB_UNIVERSAL_ESCAPE) {
        ctx.set_focus(data.widget_id);
        ctx.resign_focus();
      } else if let Some(()) = cmd.get(App::SELF_UPDATE) {
        let original_exe = std::env::current_exe();
        if dbg!(support_self_update()) && original_exe.is_ok() {
          let widget = if self_update().is_ok() {
            Modal::new("Restart?")
              .with_content("Update complete.")
              .with_content("Would you like to restart?")
              .with_button(
                "Restart",
                App::RESTART
                  .with(original_exe.as_ref().unwrap().clone())
                  .to(Target::Global),
              )
              .with_close_label("Cancel")
          } else {
            Modal::new("Error")
              .with_content("Failed to update Mod Manager.")
              .with_content("It is recommended that you restart and check that the Manager has not been corrupted.")
              .with_close()
          };

          widget.show(ctx, env, &());
        } else {
          open_in_browser();
        }
      } else if let Some(payload) = cmd.get(App::UPDATE_AVAILABLE) {
        let widget = if let Ok(release) = payload {
          let local_tag = TAG.strip_prefix('v').unwrap_or(TAG);
          let release_tag = release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name);
          if bump_is_greater(local_tag, release_tag).is_ok_and(|b| *b) {
            Modal::new("Update Mod Manager?")
              .with_content("A new version of Starsector Mod Manager is available.")
              .with_content(format!("Current version: {}", TAG))
              .with_content(format!("New version: {}", release.tag_name))
              .with_content({
                #[cfg(not(target_os = "macos"))]
                let label = "Would you like to update now?";
                #[cfg(target_os = "macos")]
                let label = "Would you like to open the update in your browser?";

                label
              })
              .with_button("Update", App::SELF_UPDATE)
              .with_close_label("Cancel")
          } else {
            return;
          }
        } else {
          Modal::new("Error")
            .with_content("Failed to retrieve Mod Manager update status.")
            .with_content("There may or may not be an update available.")
            .with_close()
        };

        widget.show(ctx, env, &());
      } else if let Some(original_exe) = cmd.get(App::RESTART) {
        if process::Command::new(original_exe).spawn().is_ok() {
          ctx.submit_command(commands::QUIT_APP)
        } else {
          eprintln!("Failed to restart")
        };
      }
      if (cmd.is(ModList::SUBMIT_ENTRY) || cmd.is(App::ENABLE)) && ctx.is_disabled() {
        ctx.set_disabled(false);
      } else if cmd.is(App::DISABLE) {
        ctx.set_disabled(true)
      }
    }

    child.event(ctx, event, data, env)
  }
}

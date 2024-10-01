use std::process;

use druid::{
  commands, widget::Controller, Command, Env, Event, EventCtx, Selector, Target, Widget,
};
use self_update::version::bump_is_greater;
use webview_shared::ExtEventSinkExt;

use crate::{
  app::{
    installer::{ChannelMessage, INSTALL, INSTALL_FOUND_MULTIPLE},
    mod_entry::UpdateStatus,
    mod_list::ModList,
    modal::Modal,
    overlays::Popup,
    settings::{self, Settings, SettingsCommand},
    updater::{open_in_browser, self_update, support_self_update},
    App, TAG,
  },
  match_command,
  nav_bar::Nav,
  widgets::root_stack::RootStack,
};

pub struct AppController;

impl<W: Widget<App>> Controller<App, W> for AppController {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut App, env: &Env) {
    if let Event::Command(cmd) = event {
      if let Some(settings::SettingsCommand::SelectInstallDir) = cmd.get(Settings::SELECTOR) {
        let ext_ctx = ctx.get_external_handle();
        ctx.set_disabled(true);
        data.runtime.spawn_blocking(move || {
          #[cfg(target_os = "macos")]
          let res = rfd::FileDialog::new()
            .add_filter("*.app", &["app"])
            .pick_file();
          #[cfg(target_os = "windows")]
          let res = rfd::FileDialog::new().pick_folder();
          #[cfg(target_os = "linux")]
          let res = native_dialog::FileDialog::new()
            .show_open_single_dir()
            .ok()
            .flatten();

          if let Some(handle) = res {
            if let Err(err) = ext_ctx.submit_command_global(
              Settings::SELECTOR,
              SettingsCommand::UpdateInstallDir(handle),
            ) {
              dbg!(err);
            }
          }
          let _ = ext_ctx.submit_command_global(App::ENABLE, ());
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
              .with_content(
                "It is recommended that you restart and check that the Manager has not been \
                 corrupted.",
              )
              .with_close()
          };

          widget.show(ctx, env, &());
        } else {
          open_in_browser();
        }
      } else if let Some(payload) = cmd.get(App::UPDATE_AVAILABLE) {
        eprintln!("got update availability");
        let _widget: Modal<'_, ()> = if let Ok(release) = payload {
          let local_tag = TAG.strip_prefix('v').unwrap_or(TAG);
          let release_tag = release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name);
          if let Ok(true) = bump_is_greater(local_tag, release_tag) {
            Modal::new("Update Mod Manager?")
              .with_content("A new version of Starsector Mod Manager is available.")
              .with_content(format!("Current version: {TAG}"))
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

        // widget.show(ctx, env, &());
      } else if let Some(original_exe) = cmd.get(App::RESTART) {
        if process::Command::new(original_exe).spawn().is_ok() {
          ctx.submit_command(commands::QUIT_APP);
        } else {
          eprintln!("Failed to restart");
        };
      } else if cmd.is(App::ENABLE) {
        ctx.set_disabled(false);
      } else if let Some(payload) = cmd.get(INSTALL) {
        match payload {
          ChannelMessage::Success(entry) => {
            let mut entry = entry.clone();
            if let Some(existing) = data.mod_list.mods.get(&entry.id) {
              entry.enabled = existing.enabled;
              if let Some(remote_version_checker) = existing.remote_version.clone() {
                entry.remote_version = Some(remote_version_checker.clone());
                entry.update_status = Some(UpdateStatus::from((
                  entry.version_checker.as_ref().unwrap(),
                  &Some(remote_version_checker),
                )));
              }
            }
            ctx.submit_command(ModList::INSERT_MOD.with(entry));
            ctx.request_update();
          }
          ChannelMessage::Duplicate(conflict, to_install, entry) => ctx.submit_command(
            App::LOG_OVERWRITE.with((conflict.clone(), to_install.clone(), entry.clone())),
          ),
          ChannelMessage::FoundMultiple(source, found_paths) => {
            ctx.submit_command(App::FOUND_MULTIPLE.with((source.clone(), found_paths.clone())));
          }
          ChannelMessage::Error(name, err) => {
            ctx.submit_command(App::LOG_ERROR.with((name.clone(), err.clone())));
            eprintln!("Failed to install {err}");
          }
        }
      }
    } else if let Event::MouseDown(_) = event {
      if ctx.is_disabled() {
        ctx.set_handled();
      }
    }

    child.event(ctx, event, data, env);
  }
}

pub struct MaskController {
  delayed_commands: Vec<Command>,
}

impl Default for MaskController {
  fn default() -> Self {
    Self::new()
  }
}

impl MaskController {
  #[must_use] pub fn new() -> Self {
    Self {
      delayed_commands: Vec::new(),
    }
  }

  fn command_whitelist(cmd: &Command) -> bool {
    const BUILTIN_TEXTBOX_CANCEL: Selector<()> =
      Selector::new("druid.builtin.textbox-cancel-editing");
    match_command!(cmd, true => {
      Popup::DISMISS,
      Popup::DISMISS_MATCHING,
      Popup::OPEN_POPUP,
      Popup::QUEUE_POPUP,
      Popup::DELAYED_POPUP,
      Popup::OPEN_NEXT,
      INSTALL_FOUND_MULTIPLE,
      ModList::OVERWRITE,
      BUILTIN_TEXTBOX_CANCEL,
      App::CONFIRM_DELETE_MOD,
      Nav::NAV_SELECTOR,
      Settings::SELECTOR,
      RootStack::SHOW,
      RootStack::DISMISS, => false,
    })
  }
}

impl<W: Widget<App>> Controller<App, W> for MaskController {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut App, env: &Env) {
    if !data.popups.is_empty()
      && let Event::Command(cmd) = event
      && !ctx.is_handled()
      && Self::command_whitelist(cmd)
    {
      self.delayed_commands.push(cmd.clone());
    }

    child.event(ctx, event, data, env);
  }

  fn lifecycle(
    &mut self,
    child: &mut W,
    ctx: &mut druid::LifeCycleCtx,
    event: &druid::LifeCycle,
    data: &App,
    env: &Env,
  ) {
    child.lifecycle(ctx, event, data, env);
  }

  fn update(
    &mut self,
    child: &mut W,
    ctx: &mut druid::UpdateCtx,
    old_data: &App,
    data: &App,
    env: &Env,
  ) {
    if data.popups.is_empty() {
      if !self.delayed_commands.is_empty() {
        for cmd in self.delayed_commands.drain(0..) {
          ctx.submit_command(cmd);
        }
      }
      if !old_data.popups.is_empty() {
        ctx.submit_command(Popup::IS_EMPTY);
      }
    }

    child.update(ctx, old_data, data, env);
  }
}

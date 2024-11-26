use std::{env::current_exe, process};

use common::ExtEventSinkExt;
use druid::{commands, widget::Controller, Env, Event, EventCtx, Widget};
use futures_util::FutureExt;
use itertools::Itertools;

use crate::{
  app::{
    installer_impl::{InstallMessage, INSTALL},
    mod_entry::UpdateStatus,
    mod_list::ModList,
    settings::{self, Settings, SettingsCommand},
    App,
  },
  bang,
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
          #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
          ))]
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
      } else if cmd.is(App::RESTART) {
        if process::Command::new(current_exe().unwrap())
          .spawn()
          .is_ok()
        {
          ctx.submit_command(commands::QUIT_APP);
        } else {
          eprintln!("Failed to restart");
        };
      } else if cmd.is(App::ENABLE) {
        ctx.set_disabled(false);
      } else if let Some(single_use) = cmd.get(INSTALL)
        && let Some(payload) = single_use.take()
      {
        match payload {
          InstallMessage::Success(entry) => {
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
            ctx.submit_command(ModList::INSERT_MOD.with(*entry));
            ctx.request_update();
          }
          InstallMessage::Error(name, err) => {
            ctx.submit_command(App::LOG_ERROR.with((name.clone(), err.clone())));
            eprintln!("Failed to install {err}");
          }
          InstallMessage::FoundMultiple(to_install, source) => {
            let install_dir = data.settings.install_dir.as_ref().unwrap().clone();
            data.runtime.spawn(
              data
                .installer
                .install(installer::Request::Initial(
                  to_install
                    .into_iter()
                    .map(|p| source.clone().with_path(&p))
                    .collect_vec(),
                  install_dir,
                ))
                .then(async move |()| drop(source)),
            );
          }
          InstallMessage::CheckConflict(id, tx) => {
            let _ = tx
              .send(data.mod_list.mods.contains_key(&id))
              .inspect_err(|err| bang!(err));
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

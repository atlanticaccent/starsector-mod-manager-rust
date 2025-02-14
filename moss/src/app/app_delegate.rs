use druid::{
  keyboard_types::Key, AppDelegate as Delegate, Command, DelegateCtx, Env, Event, Handled,
  KeyEvent, LensExt as _, Target, WindowId,
};
use installer::HybridPath;
use itertools::Itertools;
use remove_dir_all::remove_dir_all;
use updater::check_for_update;
use webview::{InstallType, PROJECT, WEBVIEW_INSTALL};

use super::{
  installer_impl::{DOWNLOAD_PROGRESS, DOWNLOAD_STARTED},
  mod_description::{self, ModDescription},
  mod_list::{install::install_options::InstallOptions, ModList},
  overlays::Popup,
  settings::{self, Settings, SettingsCommand},
  tools,
  util::{get_starsector_version, GET_INSTALLED_STARSECTOR},
  App,
};
use crate::{app::updater::get_update_status_handler, nav_bar::Nav};

pub enum AppCommands {
  UpdateModDescription(ModDescription<String>),
  PickFile(bool),
}

pub struct AppDelegate {
  pub root_id: Option<WindowId>,
}

impl AppDelegate {
  pub fn new() -> Self {
    Self { root_id: None }
  }
}

impl Delegate<App> for AppDelegate {
  #[allow(clippy::too_many_lines)]
  fn command(
    &mut self,
    ctx: &mut DelegateCtx,
    _target: Target,
    cmd: &Command,
    data: &mut App,
    _env: &Env,
  ) -> Handled {
    if cmd.is(App::SELECTOR) {
      match cmd.get_unchecked(App::SELECTOR) {
        AppCommands::UpdateModDescription(desc) => {
          data.active = Some(desc.clone());

          return Handled::Yes;
        }
        AppCommands::PickFile(is_file) => {
          let sink = ctx.get_external_handle();
          if *is_file {
            data.runtime.spawn_blocking(move || {
              #[cfg(not(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
              )))]
              let res = rfd::FileDialog::new()
                .add_filter("Archives", &[
                  "zip", "7z", "7zip", "rar", "rar4", "rar5", "tar",
                ])
                .pick_files();
              #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
              ))]
              let res = native_dialog::FileDialog::new()
                .add_filter("Archives", &[
                  "zip", "7z", "7zip", "rar", "rar4", "rar5", "tar",
                ])
                .show_open_multiple_file()
                .ok();

              sink.submit_command(App::OPEN_FILE, res, Target::Auto)
            });
          } else {
            data.runtime.spawn_blocking(move || {
              #[cfg(not(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
              )))]
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

              sink.submit_command(App::OPEN_FILE, res.map(|folder| vec![folder]), Target::Auto)
            });
          }
        }
      }
    } else if let Some(SettingsCommand::UpdateInstallDir(new_install_dir)) =
      cmd.get(settings::Settings::SELECTOR)
    {
      if data.settings.install_dir != Some(new_install_dir.clone()) || data.settings.dirty {
        data.settings.install_dir_buf = new_install_dir.to_string_lossy().to_string();
        data.settings.install_dir = Some(new_install_dir.clone());
        data.settings.vmparams =
          tools::vmparams::VMParams::load(new_install_dir, data.settings.vmparams_linked).ok();

        data.runtime.spawn(get_starsector_version(
          ctx.get_external_handle(),
          new_install_dir.clone(),
        ));
        if !data.settings.dirty || data.mod_list.mods.is_empty() {
          data.mod_list.mods.clear();
          data.runtime.spawn(ModList::parse_mod_folder_async(
            new_install_dir.clone(),
            ctx.get_external_handle(),
          ));
        }
        data.settings.dirty = false;

        if data.settings.save().is_err() {
          eprintln!("Failed to save settings");
        };
        data.mod_list.install_dir_available = true;
      }
      return Handled::No;
    } else if let Some(entry) = cmd.get(ModList::AUTO_UPDATE) {
      ctx.submit_command(App::LOG_MESSAGE.with(format!("Begin auto-update of {}", entry.name)));
    } else if let Some(()) = cmd.get(App::REFRESH) {
      if let Some(install_dir) = data.settings.install_dir.as_ref() {
        data.runtime.spawn(ModList::parse_mod_folder_async(
          install_dir.clone(),
          ctx.get_external_handle(),
        ));
      }

      return Handled::No;
    } else if let Some(res) = cmd.get(GET_INSTALLED_STARSECTOR) {
      App::mod_list
        .then(ModList::starsector_version)
        .put(data, res.as_ref().ok().cloned());
    } else if let Some(single_use) = cmd.get(App::LOG_OVERWRITE)
      && let Some((conflict, to_install, entry)) = single_use.take()
    {
      ctx.submit_command(Popup::QUEUE_POPUP.with(Popup::overwrite(conflict, to_install, entry)));

      return Handled::Yes;
    } else if let Some(install) = cmd.get(WEBVIEW_INSTALL)
      && let Some(install_type) = install.take()
    {
      let installer = data.installer.clone();
      let install_dir = data.settings.install_dir.clone().unwrap();

      data.runtime.spawn(async move {
        let temp_file = match install_type {
          InstallType::Uri(uri) => {
            installer::InstallerExt::download_in(&installer, &uri, Some(PROJECT.cache_dir()))
              .await
              .expect("Download archive")
          }
          InstallType::Path(path) => path,
        };

        let path = temp_file.path();
        installer
          .install(installer::Request::Initial(
            vec![HybridPath::PathBuf(path.to_owned())],
            install_dir,
          ))
          .await;
      });

      return Handled::Yes;
    } else if let Some(url) = cmd.get(mod_description::OPEN_IN_BROWSER) {
      if data.settings.open_forum_link_in_webview {
        ctx.submit_command(Nav::NAV_SELECTOR.with(crate::nav_bar::NavLabel::WebBrowser));
        ctx.submit_command(App::OPEN_WEBVIEW.with(Some(url.clone())));
      } else {
        let _ = opener::open(url);
      }
    } else if let Some(url) = cmd.get(App::OPEN_EXTERNALLY) {
      if opener::open(url).is_err() {
        eprintln!("Failed to open GitHub");
      }
    } else if let Some(entry) = cmd.get(App::CONFIRM_DELETE_MOD) {
      if remove_dir_all(&entry.path).is_ok() {
        data.mod_list.mods.remove(&entry.mod_id);
        data.active = None;
      } else {
        eprintln!("Failed to delete mod");
      }
    } else if let Some((_timestamp, _url)) = cmd.get(DOWNLOAD_STARTED) {
      // data
      //   .downloads
      //   .insert(*timestamp, (*timestamp, url.clone(), 0.0));

      return Handled::Yes;
    } else if let Some(_updates) = cmd.get(DOWNLOAD_PROGRESS) {
      // for update in updates {
      //   data.downloads.insert(update.0, update.clone());
      // }

      return Handled::Yes;
    } else if let Some(_timestamp) = cmd.get(App::REMOVE_DOWNLOAD_BAR) {
      // data.downloads.remove(timestamp);

      return Handled::Yes;
    }
    if let Some(Some(targets)) = cmd.get(App::OPEN_FILE) {
      if !targets.is_empty() {
        ctx.submit_command(App::LOG_MESSAGE.with(format!("Installing {}",
          targets
            .iter()
            .map(|t| {
              t.file_name().map_or_else(
                || String::from("unknown"),
                |f| f.to_string_lossy().into_owned(),
              )
            })
            .collect::<Vec<String>>()
            .join(", "),
        )));
        data
          .runtime
          .spawn(data.installer.install(installer::Request::Initial(
            targets.iter().map(|p| p.clone().into()).collect_vec(),
            data.settings.install_dir.clone().unwrap(),
          )));
      }
      return Handled::Yes;
    }

    Handled::No
  }

  #[allow(unused_variables)]
  fn window_removed(&mut self, id: WindowId, data: &mut App, _env: &Env, ctx: &mut DelegateCtx) {
    match Some(id) {
      a if a == self.root_id => {
        println!("quitting");

        drop(data.browser.inner.take());

        let _ = std::fs::remove_dir_all(PROJECT.cache_dir());
        #[cfg(not(target_os = "macos"))]
        ctx.submit_command(druid::commands::QUIT_APP);
        #[cfg(target_os = "macos")]
        std::process::exit(0);
      }
      _ => {}
    }
  }

  fn event(
    &mut self,
    ctx: &mut DelegateCtx,
    window_id: WindowId,
    event: druid::Event,
    data: &mut App,
    _: &Env,
  ) -> Option<druid::Event> {
    match event {
      Event::WindowConnected => {
        if self.root_id.is_none() {
          self.root_id = Some(window_id);
          if data.settings.dirty
            && let Some(install_dir) = data.settings.install_dir.as_ref()
          {
            ctx.submit_command(
              Settings::SELECTOR.with(SettingsCommand::UpdateInstallDir(install_dir.clone())),
            );
          }
          check_for_update(get_update_status_handler(ctx.get_external_handle()));

          let mut delayed_popups = Vec::new();
          if data
            .settings
            .install_dir
            .as_ref()
            .is_none_or(|p| !p.exists())
          {
            delayed_popups.push(Popup::SelectInstall);
          }

          ctx.submit_command(Popup::DELAYED_POPUP.with(delayed_popups));
        }
      }
      Event::KeyDown(KeyEvent {
        key: Key::Escape, ..
      }) => {
        ctx.submit_command(App::DUMB_UNIVERSAL_ESCAPE);
      }
      Event::MouseDown(ref mouse) => {
        ctx.submit_command(InstallOptions::DISMISS.with(mouse.window_pos));
      }
      _ => {}
    }

    Some(event)
  }
}

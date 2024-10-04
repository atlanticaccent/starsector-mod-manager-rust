use druid::{
  keyboard_types::Key, AppDelegate as Delegate, Command, DelegateCtx, Env, Event, Handled,
  KeyEvent, LensExt as _, SingleUse, Target, WindowHandle, WindowId,
};
use itertools::Itertools;
use rand::random;
use remove_dir_all::remove_dir_all;
use reqwest::Url;
use webview_shared::{InstallType, PROJECT, WEBVIEW_INSTALL};

use super::{
  installer::{self, DOWNLOAD_PROGRESS, DOWNLOAD_STARTED},
  mod_description::{self, ModDescription},
  mod_list::{install::install_options::InstallOptions, ModList},
  overlays::Popup,
  settings::{self, Settings, SettingsCommand},
  tools,
  util::{get_starsector_version, GET_INSTALLED_STARSECTOR},
  App,
};
use crate::{app::updater::check_for_update, nav_bar::Nav};

pub enum AppCommands {
  UpdateModDescription(ModDescription<String>),
  PickFile(bool),
}

#[derive(Default)]
pub struct AppDelegate {
  pub root_id: Option<WindowId>,
  pub root_window: Option<WindowHandle>,
  pub startup_popups: Vec<Popup>,
}

impl Delegate<App> for AppDelegate {
  fn window_added(
    &mut self,
    _id: WindowId,
    handle: druid::WindowHandle,
    _data: &mut App,
    _env: &Env,
    _ctx: &mut DelegateCtx,
  ) {
    if self.root_window.is_none() {
      self.root_window = Some(handle);
    }
  }

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
              #[cfg(not(linux))]
              let res = rfd::FileDialog::new()
                .add_filter("Archives", &[
                  "zip", "7z", "7zip", "rar", "rar4", "rar5", "tar",
                ])
                .pick_files();
              #[cfg(linux)]
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
              #[cfg(not(linux))]
              let res = rfd::FileDialog::new().pick_folder();
              #[cfg(linux)]
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
    } else if let Some((conflict, to_install, entry)) = cmd.get(App::LOG_OVERWRITE) {
      ctx.submit_command(Popup::QUEUE_POPUP.with(Popup::overwrite(
        conflict.clone(),
        to_install.clone(),
        entry.clone(),
      )));

      return Handled::Yes;
    } else if let Some(install) = cmd.get(WEBVIEW_INSTALL) {
      let runtime = data.runtime.clone();
      let install = install.clone();
      let ext_ctx = ctx.get_external_handle();
      let install_dir = data.settings.install_dir.clone().unwrap();
      let ids = data.mod_list.mods.values().map(|v| v.id.clone()).collect();
      data.runtime.spawn_blocking(move || {
        runtime.block_on(async move {
          let path = match install {
            InstallType::Uri(uri) => {
              let file_name = Url::parse(&uri)
                .ok()
                .and_then(|url| {
                  url
                    .path_segments()
                    .and_then(std::iter::Iterator::last)
                    .map(std::string::ToString::to_string)
                })
                .unwrap_or_else(|| uri.clone())
                .to_string();
              ext_ctx
                .submit_command(
                  App::LOG_MESSAGE,
                  format!("Installing {}", &file_name),
                  Target::Auto,
                )
                .expect("Send install start");
              let download = installer::download(uri, ext_ctx.clone())
                .await
                .expect("Download archive");
              let download_dir = PROJECT.cache_dir().to_path_buf();
              let mut persist_path = download_dir.join(&file_name);
              if persist_path.exists() {
                persist_path = download_dir.join(format!("{}({})", file_name, random::<u8>()));
              }
              if let Err(err) = download.persist(&persist_path) {
                if err.error.kind() == std::io::ErrorKind::CrossesDevices {
                  std::fs::copy(err.file.path(), &persist_path)
                    .expect("Copy download across devices");
                } else {
                  panic!("{}", err)
                }
              }

              persist_path
            }
            InstallType::Path(path) => {
              let file_name = path
                .file_name()
                .unwrap_or(path.as_os_str())
                .to_string_lossy()
                .to_string();
              ext_ctx
                .submit_command(
                  App::LOG_MESSAGE,
                  format!("Installing {}", &file_name),
                  Target::Auto,
                )
                .expect("Send install start");

              path
            }
          };
          installer::Payload::Initial(vec![path.into()])
            .install(ext_ctx, install_dir, ids)
            .await;
        });
      });
      return Handled::Yes;
    } else if let Some(url) = cmd.get(mod_description::OPEN_IN_BROWSER) {
      if data.settings.open_forum_link_in_webview {
        ctx.submit_command(Nav::NAV_SELECTOR.with(crate::nav_bar::NavLabel::WebBrowser));
        ctx.submit_command(App::OPEN_WEBVIEW.with(Some(url.clone())));
      } else {
        let _ = opener::open(url);
      }
    } else if let Some(entry) = cmd.get(App::CONFIRM_DELETE_MOD) {
      if remove_dir_all(&entry.path).is_ok() {
        data.mod_list.mods.remove(&entry.id);
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
    } else if let Some((to_install, source)) = cmd
      .get(installer::INSTALL_FOUND_MULTIPLE)
      .and_then(SingleUse::take)
    {
      let ext_ctx = ctx.get_external_handle();
      let install_dir = data.settings.install_dir.as_ref().unwrap().clone();
      let ids = data.mod_list.mods.values().map(|v| v.id.clone()).collect();
      data.runtime.spawn(async move {
        installer::Payload::Initial(
          to_install
            .into_iter()
            .map(|p| source.clone().with_path(&p))
            .collect_vec(),
        )
        .install(ext_ctx, install_dir, ids)
        .await;

        drop(source);
      });

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
        data.runtime.spawn(
          installer::Payload::Initial(targets.iter().map(|p| p.clone().into()).collect_vec())
            .install(
              ctx.get_external_handle(),
              data.settings.install_dir.clone().unwrap(),
              data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
            ),
        );
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
        if let Some(child) = &data.browser.inner {
          data.browser.inner = None;
        }
        let _ = std::fs::remove_dir_all(PROJECT.cache_dir());
        #[cfg(not(mac))]
        ctx.submit_command(druid::commands::QUIT_APP);
        #[cfg(mac)]
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
          let ext_ctx = ctx.get_external_handle();
          data.runtime.spawn(check_for_update(ext_ctx));

          let mut delayed_popups = Vec::new();
          if data
            .settings
            .install_dir
            .as_ref()
            .is_none_or(|p| !p.exists())
          {
            delayed_popups.push(Popup::SelectInstall);
          }
          delayed_popups.append(&mut self.startup_popups);

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

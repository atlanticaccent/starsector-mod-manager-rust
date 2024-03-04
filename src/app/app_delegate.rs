use std::{
  fs::{metadata, File},
  io::Write as _,
  path::PathBuf,
  rc::Rc,
  sync::Arc,
};

use base64::{decode, encode};
use chrono::{DateTime, Local, TimeZone as _};
use druid::{
  commands,
  im::Vector,
  keyboard_types::Key,
  lens,
  widget::{Button, Either, Flex, Label, List, Maybe, Scope, Spinner, ViewSwitcher},
  AppDelegate as Delegate, Command, DelegateCtx, Env, Event, EventCtx, Handled, KeyEvent,
  LensExt as _, SingleUse, Size, Target, Widget, WidgetExt as _, WindowDesc, WindowHandle,
  WindowId, WindowLevel,
};
use druid_widget_nursery::{material_icons::Icon, ProgressBar, Separator};
use rand::random;
use remove_dir_all::remove_dir_all;
use reqwest::Url;
use tap::Pipe as _;
use util::{CLOSE, VERIFIED};
use webview_shared::{
  InstallType, UserEvent, PROJECT, WEBVIEW_EVENT, WEBVIEW_INSTALL, WEBVIEW_OFFSET,
};
use webview_subsystem::init_webview;

use super::{
  controllers::HoverController,
  installer,
  installer::{HybridPath, StringOrPath, DOWNLOAD_PROGRESS, DOWNLOAD_STARTED, INSTALL_ALL},
  mod_description,
  mod_entry::{ModEntry, ModMetadata},
  mod_list::{install::install_options::InstallOptions, ModList},
  modal::Modal,
  settings,
  settings::{Settings, SettingsCommand},
  util,
  util::{
    get_latest_manager, get_starsector_version, Button2, CommandExt as _, DummyTransfer,
    LabelExt as _, WidgetExtEx as _, GET_INSTALLED_STARSECTOR,
  },
  App,
};

pub enum AppCommands {
  UpdateModDescription(String),
  PickFile(bool),
}

#[derive(Default)]
pub struct AppDelegate {
  pub settings_id: Option<WindowId>,
  pub root_id: Option<WindowId>,
  pub root_window: Option<WindowHandle>,
  pub log_window: Option<WindowId>,
  pub overwrite_window: Option<WindowId>,
  pub duplicate_window: Option<WindowId>,
  pub download_window: Option<WindowId>,
  pub mega_file: Option<(File, PathBuf)>,
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
        },
        AppCommands::PickFile(is_file) => {
          let sink = ctx.get_external_handle();
          if *is_file {
            data.runtime.spawn_blocking(move || {
              #[cfg(not(target_os = "linux"))]
              let res = rfd::FileDialog::new()
                .add_filter(
                  "Archives",
                  &["zip", "7z", "7zip", "rar", "rar4", "rar5", "tar"],
                )
                .pick_files();
              #[cfg(target_os = "linux")]
              let res = native_dialog::FileDialog::new()
                .add_filter(
                  "Archives",
                  &["zip", "7z", "7zip", "rar", "rar4", "rar5", "tar"],
                )
                .show_open_multiple_file()
                .ok();

              sink.submit_command(App::OPEN_FILE, res, Target::Auto)
            });
          } else {
            data.runtime.spawn_blocking(move || {
              #[cfg(not(target_os = "linux"))]
              let res = rfd::FileDialog::new().pick_folder();
              #[cfg(target_os = "linux")]
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

        data.runtime.spawn(get_starsector_version(
          ctx.get_external_handle(),
          new_install_dir.clone(),
        ));
        if !data.settings.dirty {
          data.mod_list.mods.clear();
          data.runtime.spawn(ModList::parse_mod_folder(
            Some(ctx.get_external_handle()),
            Some(new_install_dir.clone()),
          ));
        }
        data.settings.dirty = false;

        if data.settings.save().is_err() {
          eprintln!("Failed to save settings")
        };
        data.mod_list.install_dir_available = true
      }
      return Handled::Yes;
    } else if let Some(entry) = cmd.get(ModList::AUTO_UPDATE) {
      ctx.submit_command(App::LOG_MESSAGE.with(format!("Begin auto-update of {}", entry.name)));
      data
        .runtime
        .spawn(installer::Payload::Download(entry.clone()).install(
          ctx.get_external_handle(),
          data.settings.install_dir.clone().unwrap(),
          data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
        ));
    } else if let Some(()) = cmd.get(App::REFRESH) {
      if let Some(install_dir) = data.settings.install_dir.as_ref() {
        // data.mod_list.mods.clear();
        data.runtime.spawn(ModList::parse_mod_folder(
          Some(ctx.get_external_handle()),
          Some(install_dir.clone()),
        ));
      }

      return Handled::Yes;
    } else if let Some(res) = cmd.get(GET_INSTALLED_STARSECTOR) {
      App::mod_list
        .then(ModList::starsector_version)
        .put(data, res.as_ref().ok().cloned());
    } else if let Some(name) = cmd.get(App::LOG_SUCCESS) {
      data.log_message(&format!("Successfully installed {}", name));
      self.display_if_closed(ctx, SubwindowType::Log);

      return Handled::Yes;
    } else if let Some(()) = cmd.get(App::CLEAR_LOG) {
      data.log.clear();

      return Handled::Yes;
    } else if let Some((name, err)) = cmd.get(App::LOG_ERROR) {
      data.log_message(&format!("Failed to install {}. Error: {}", name, err));
      self.display_if_closed(ctx, SubwindowType::Log);

      return Handled::Yes;
    } else if let Some(message) = cmd.get(App::LOG_MESSAGE) {
      data.log_message(message);
      self.display_if_closed(ctx, SubwindowType::Log);

      return Handled::Yes;
    } else if let Some(message) = cmd.get(App::LOG_OVERWRITE) {
      data.push_overwrite(message.clone());
      self.display_if_closed(ctx, SubwindowType::Overwrite);

      return Handled::Yes;
    } else if let Some(ovewrite_all) = cmd.get(App::CLEAR_OVERWRITE_LOG) {
      if *ovewrite_all {
        for val in &data.overwrite_log {
          let (conflict, to_install, entry) = val.as_ref();
          ctx.submit_command(ModList::OVERWRITE.with((
            match conflict {
              StringOrPath::String(id) => data.mod_list.mods.get(id).unwrap().path.clone(),
              StringOrPath::Path(path) => path.clone(),
            },
            to_install.clone(),
            entry.clone(),
          )))
        }
      }
      data.overwrite_log.clear();

      return Handled::Yes;
    } else if let Some(overwrite_entry) = cmd.get(App::REMOVE_OVERWRITE_LOG_ENTRY) {
      data.overwrite_log.retain(|val| val.0 != *overwrite_entry);
      if data.overwrite_log.is_empty() {
        if let Some(id) = self.overwrite_window.take() {
          ctx.submit_command(commands::CLOSE_WINDOW.to(id))
        }
      }

      return Handled::Yes;
    } else if let Some(duplicates) = cmd.get(ModList::DUPLICATE) {
      data.push_duplicate(duplicates);
      self.display_if_closed(ctx, SubwindowType::Duplicate);

      return Handled::Yes;
    } else if let Some((delete_path, keep_entry)) = cmd.get(App::DELETE_AND_SUMBIT) {
      let ext_ctx = ctx.get_external_handle();
      let delete_path = delete_path.clone();
      let keep_entry = keep_entry.clone();
      data.runtime.spawn(async move {
        if remove_dir_all(delete_path).is_ok() {
          let remote_version = keep_entry.version_checker.clone();
          if ext_ctx
            .submit_command(ModEntry::REPLACE, keep_entry, Target::Auto)
            .is_err()
          {
            eprintln!("Failed to submit new entry")
          };
          if let Some(version_meta) = remote_version {
            util::get_master_version(&reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(500))
            .connect_timeout(std::time::Duration::from_millis(500))
            .build()
            .expect("Build reqwest client"), Some(ext_ctx), version_meta).await;
          }
        } else {
          eprintln!("Failed to delete duplicate mod");
        }
      });

      return Handled::Yes;
    } else if let Some(id) = cmd.get(App::REMOVE_DUPLICATE_LOG_ENTRY) {
      data.duplicate_log.retain(|entry| entry.0.id != *id);
      if data.duplicate_log.is_empty() {
        if let Some(id) = self.duplicate_window.take() {
          ctx.submit_command(commands::CLOSE_WINDOW.to(id))
        }
      }

      return Handled::Yes;
    } else if let Some(()) = cmd.get(App::CLEAR_DUPLICATE_LOG) {
      data.duplicate_log.clear();
      if let Some(id) = self.duplicate_window.take() {
        ctx.submit_command(commands::CLOSE_WINDOW.to(id))
      }

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
                    .and_then(|segments| segments.last())
                    .map(|s| s.to_string())
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
                persist_path = download_dir.join(format!("{}({})", file_name, random::<u8>()))
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
          installer::Payload::Initial(vec![path])
            .install(ext_ctx, install_dir, ids)
            .await;
        });
      });
      return Handled::Yes;
    } else if let Some(url) = cmd.get(App::OPEN_WEBVIEW)
      && let Some(window) = self.root_window.as_ref()
    {
      ctx.submit_command(App::DISABLE);
      let webview =
        init_webview(url.clone(), window, ctx.get_external_handle()).expect("Initialize webview");

      data.webview = Some(Rc::new(webview))
    } else if let Some(url) = cmd.get(mod_description::OPEN_IN_BROWSER) {
      if data.settings.open_forum_link_in_webview {
        ctx.submit_command(App::OPEN_WEBVIEW.with(Some(url.clone())));
      } else {
        let _ = opener::open(url);
      }
    } else if let Some(entry) = cmd.get(ModEntry::ASK_DELETE_MOD) {
      let modal = Modal::<App>::new(&format!("Delete {}", entry.name))
        .with_content(format!("Do you want to PERMANENTLY delete {}?", entry.name))
        .with_content("This operation cannot be undone.")
        .with_button("Confirm", App::CONFIRM_DELETE_MOD.with(entry.clone()))
        .with_close_label("Cancel")
        .build();

      let window = WindowDesc::new(modal)
        .window_size((400., 150.))
        .show_titlebar(false)
        .set_level(WindowLevel::AppWindow);

      ctx.new_window(window)
    } else if let Some(entry) = cmd.get(App::CONFIRM_DELETE_MOD) {
      if remove_dir_all(&entry.path).is_ok() {
        data.mod_list.mods.remove(&entry.id);
      } else {
        eprintln!("Failed to delete mod")
      }
    } else if let Some((timestamp, url)) = cmd.get(DOWNLOAD_STARTED) {
      data
        .downloads
        .insert(*timestamp, (*timestamp, url.clone(), 0.0));

      self.display_if_closed(ctx, SubwindowType::Download);

      return Handled::Yes;
    } else if let Some(updates) = cmd.get(DOWNLOAD_PROGRESS) {
      for update in updates {
        data.downloads.insert(update.0, update.clone());
      }

      self.display_if_closed(ctx, SubwindowType::Download);

      return Handled::Yes;
    } else if let Some(timestamp) = cmd.get(App::REMOVE_DOWNLOAD_BAR) {
      data.downloads.remove(timestamp);

      if data.downloads.is_empty() {
        if let Some(id) = self.download_window.take() {
          ctx.submit_command(commands::CLOSE_WINDOW.to(id))
        }
      }

      return Handled::Yes;
    } else if let Some((source, found_paths)) = cmd.get(App::FOUND_MULTIPLE) {
      let modal = Self::build_found_multiple(source.clone(), found_paths.clone());

      let window = WindowDesc::new(modal)
        .window_size((500., 400.))
        .show_titlebar(false)
        .set_level(WindowLevel::AppWindow);

      ctx.new_window(window);

      return Handled::Yes;
    } else if let Some((to_install, source)) =
      cmd.get(installer::INSTALL_ALL).and_then(SingleUse::take)
    {
      let ext_ctx = ctx.get_external_handle();
      let install_dir = data.settings.install_dir.as_ref().unwrap().clone();
      let ids = data.mod_list.mods.values().map(|v| v.id.clone()).collect();
      data.runtime.spawn(async move {
        installer::Payload::Initial(to_install.into_iter().collect())
          .install(ext_ctx, install_dir, ids)
          .await;

        drop(source);
      });

      return Handled::Yes;
    } else if let Some(user_event) = cmd.get(WEBVIEW_EVENT)
      && let Some(webview) = &data.webview
    {
      match user_event {
        UserEvent::Navigation(uri) => {
          println!("Navigation: {}", uri);
          if uri.starts_with("https://www.mediafire.com/file") {
            let _ = webview.evaluate_script(r#"window.alert("You appear to be on a Mediafire site.\nIn order to correctly trigger a Mediafire download, attempt to open the dowload link in a new window.\nThis can be done through the right click context menu, or using a platform shortcut.")"#);
          }
        }
        UserEvent::AskDownload(uri) => {
          #[cfg(not(target_os = "macos"))]
              let _ = webview.evaluate_script(&format!(r"
          let res = window.confirm('Detected an attempted download.\nDo you want to try and install a mod using this download?')
          window.ipc.postMessage(`confirm_download:${{res}},uri:{}`)
          ", encode(uri)));
          #[cfg(target_os = "macos")]
              let _ = webview.evaluate_script(&format!(r"
          let dialog = new Dialog();
          let res = dialog.confirm('Detected an attempted download.\nDo you want to try and install a mod using this download?', {{}})
            .then(res => window.ipc.postMessage(`confirm_download:${{res}},uri:{}`))
          ", encode(uri)));
        }
        UserEvent::Download(uri) => {
          let _ = webview.evaluate_script("location.reload();");
          ctx.submit_command(WEBVIEW_INSTALL.with(InstallType::Uri(uri.clone())))
        }
        UserEvent::CancelDownload => {}
        UserEvent::NewWindow(uri) => {
          webview
            .evaluate_script(&format!("window.location.assign('{}')", uri))
            .expect("Navigate webview");
        }
        UserEvent::BlobReceived(uri) => {
          let path = PROJECT.cache_dir().join(format!("{}", random::<u16>()));
          self.mega_file = Some((File::create(&path).expect("Create file"), path));
          webview
            .evaluate_script(&format!(
              r#"
          (() => {{
            /**
            * @type Blob
            */
            let blob = URL.getObjectURLDict()['{}']
              || Object.values(URL.getObjectURLDict())[0]

            var increment = 1024;
            var index = 0;
            var reader = new FileReader();
            let func = function() {{
              let res = reader.result;
              window.ipc.postMessage(`${{res}}`);
              index += increment;
              if (index < blob.size) {{
                let slice = blob.slice(index, index + increment);
                reader = new FileReader();
                reader.onloadend = func;
                reader.readAsDataURL(slice);
              }} else {{
                window.ipc.postMessage('#EOF');
              }}
            }};
            reader.onloadend = func;
            reader.readAsDataURL(blob.slice(index, increment))
          }})();
          "#,
              uri
            ))
            .expect("Eval script");
        }
        UserEvent::BlobChunk(chunk) => {
          if let Some((file, path)) = self.mega_file.as_mut() {
            match chunk {
              Some(chunk) => {
                let split = chunk.split(',').nth(1);
                println!("{:?}", chunk.split(',').next());
                if let Some(split) = split {
                  if let Ok(decoded) = decode(split) {
                    if file.write(&decoded).is_err() {
                      eprintln!("Failed to write bytes to temp file")
                    }
                  }
                }
              }
              None => {
                ctx.submit_command(WEBVIEW_INSTALL.with(InstallType::Path(path.clone())));
                self.mega_file = None;
              }
            }
          }
        }
      }
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
          installer::Payload::Initial(targets.iter().cloned().collect()).install(
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
      a if a == self.settings_id => self.settings_id = None,
      a if a == self.log_window => self.log_window = None,
      a if a == self.overwrite_window => {
        data.overwrite_log.clear();
        self.overwrite_window = None;
      }
      a if a == self.duplicate_window => self.duplicate_window = None,
      a if a == self.download_window => {
        data.downloads.clear();
        self.download_window = None;
      }
      a if a == self.root_id => {
        println!("quitting");
        if let Some(child) = &data.webview {
          data.webview = None;
        }
        let _ = std::fs::remove_dir_all(PROJECT.cache_dir());
        #[cfg(not(target_os = "macos"))]
        ctx.submit_command(commands::QUIT_APP);
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
          if data.settings.dirty {
            ctx.submit_command(Settings::SELECTOR.with(SettingsCommand::UpdateInstallDir(
              data.settings.install_dir.clone().unwrap_or_default(),
            )));
          }
          let ext_ctx = ctx.get_external_handle();
          data.runtime.spawn(async move {
            let release = get_latest_manager().await;
            ext_ctx.submit_command(App::UPDATE_AVAILABLE, release, Target::Auto)
          });
        }
      }
      Event::KeyDown(KeyEvent {
        key: Key::Escape, ..
      }) => {
        ctx.submit_command(App::DUMB_UNIVERSAL_ESCAPE);
      }
      Event::WindowSize(Size { width, height }) => {
        if Some(window_id) == self.root_id
          && let Some(webview) = &data.webview
        {
          webview.set_bounds(wry::Rect {
            x: 0,
            y: WEBVIEW_OFFSET.into(),
            width: width as u32,
            height: height as u32,
          })
        }
      },
      Event::MouseDown(ref mouse) => {
        ctx.submit_command(InstallOptions::DISMISS.with(mouse.window_pos))
      }
      _ => {}
    }

    Some(event)
  }
}

impl AppDelegate {
  pub fn build_log_window() -> impl Widget<App> {
    let modal = Modal::new("Log").with_content("").with_content(
      List::new(|| Label::wrapped_func(|val: &String, _| val.clone()))
        .lens(App::log)
        .boxed(),
    );

    modal.with_button("Close", App::CLEAR_LOG).build().boxed()
  }

  pub fn display_if_closed(&mut self, ctx: &mut DelegateCtx, window_type: SubwindowType) {
    let window_id = match window_type {
      SubwindowType::Log => &mut self.log_window,
      SubwindowType::Overwrite => &mut self.overwrite_window,
      SubwindowType::Duplicate => &mut self.duplicate_window,
      SubwindowType::Download => &mut self.download_window,
    };

    if let Some(id) = window_id {
      ctx.submit_command(commands::SHOW_WINDOW.to(*id))
    } else {
      let modal = match window_type {
        SubwindowType::Log => AppDelegate::build_log_window().boxed(),
        SubwindowType::Overwrite => AppDelegate::build_overwrite_window().boxed(),
        SubwindowType::Duplicate => AppDelegate::build_duplicate_window().boxed(),
        SubwindowType::Download => AppDelegate::build_progress_bars().boxed(),
      };

      let window = WindowDesc::new(modal)
        .window_size((500., 400.))
        .show_titlebar(false)
        .set_level(WindowLevel::AppWindow);

      window_id.replace(window.id);

      ctx.new_window(window);
    }
  }

  pub fn build_overwrite_window() -> impl Widget<App> {
    ViewSwitcher::new(
      |data: &App, _| data.overwrite_log.len(),
      |_, data: &App, _| {
        let mut modal = Modal::new("Overwrite?");

        for val in data.overwrite_log.iter() {
          let (conflict, to_install, entry) = val.as_ref();
          modal = modal
            .with_content(match conflict {
              StringOrPath::String(id) => {
                format!("A mod with ID {} alread exists.", id)
              }
              StringOrPath::Path(path) => format!(
                "Found a folder at the path {} when trying to install {}.",
                path.to_string_lossy(),
                entry.id
              ),
            })
            .with_content(
              Maybe::or_empty(|| {
                Label::wrapped(
                  "\
              NOTE: A .git directory has been detected in the target directory. \
              Are you sure this isn't being used for development?\
            ",
                )
              })
              .lens(lens::Constant(
                data
                  .settings
                  .git_warn
                  .then(|| {
                    if entry.path.join(".git").exists() {
                      Some(())
                    } else {
                      None
                    }
                  })
                  .flatten(),
              ))
              .boxed(),
            )
            .with_content(format!(
              "Would you like to replace the existing {}?",
              if let StringOrPath::String(_) = conflict {
                "mod"
              } else {
                "folder"
              }
            ))
            .with_content(
              Flex::row()
                .with_flex_spacer(1.)
                .with_child(Button::new("Overwrite").on_click({
                  let conflict = conflict.clone();
                  let to_install = to_install.clone();
                  let entry = entry.clone();
                  move |ctx: &mut EventCtx, data: &mut App, _| {
                    ctx.submit_command(
                      App::REMOVE_OVERWRITE_LOG_ENTRY
                        .with(conflict.clone())
                        .to(Target::Global),
                    );
                    ctx.submit_command(
                      ModList::OVERWRITE
                        .with((
                          match &conflict {
                            StringOrPath::String(id) => {
                              data.mod_list.mods.get(id).unwrap().path.clone()
                            }
                            StringOrPath::Path(path) => path.clone(),
                          },
                          to_install.clone(),
                          entry.clone(),
                        ))
                        .to(Target::Global),
                    );
                  }
                }))
                .with_child(Button::new("Cancel").on_click({
                  let conflict = conflict.clone();
                  move |ctx, _, _| {
                    ctx.submit_command(App::REMOVE_OVERWRITE_LOG_ENTRY.with(conflict.clone()));
                  }
                }))
                .boxed(),
            )
            .with_content(
              Separator::new()
                .with_width(2.0)
                .with_color(druid::Color::GRAY)
                .padding((0., 0., 0., 10.))
                .boxed(),
            );
        }

        if data.overwrite_log.len() > 1 {
          modal
            .with_button("Overwrite All", App::CLEAR_OVERWRITE_LOG.with(true))
            .with_button("Cancel All", App::CLEAR_OVERWRITE_LOG.with(false))
        } else {
          modal.with_button("Close", App::CLEAR_OVERWRITE_LOG.with(false))
        }
        .build()
        .boxed()
      },
    )
  }

  pub fn build_duplicate_window() -> impl Widget<App> {
    ViewSwitcher::new(
      |app: &App, _| app.duplicate_log.len(),
      |_, app, _| {
        Modal::new("Duplicate detected")
          .pipe(|mut modal| {
            for (dupe_a, dupe_b) in &app.duplicate_log {
              modal = modal
                .with_content(format!(
                  "Detected duplicate installs of mod with ID {}.",
                  dupe_a.id
                ))
                .with_content(
                  Flex::row()
                    .with_flex_child(Self::make_dupe_col(dupe_a, dupe_b), 1.)
                    .with_flex_child(Self::make_dupe_col(dupe_b, dupe_a), 1.)
                    .boxed(),
                )
                .with_content(
                  Flex::row()
                    .with_flex_spacer(1.)
                    .with_child(Button::new("Ignore").on_click({
                      let id = dupe_a.id.clone();
                      move |ctx, _, _| {
                        ctx.submit_command(
                          App::REMOVE_DUPLICATE_LOG_ENTRY
                            .with(id.clone())
                            .to(Target::Global),
                        )
                      }
                    }))
                    .boxed(),
                )
                .with_content(Separator::new().padding((0., 0., 0., 10.)).boxed())
            }
            modal
          })
          .with_button("Ignore All", App::CLEAR_DUPLICATE_LOG)
          .build()
          .boxed()
      },
    )
  }

  pub fn make_dupe_col(dupe_a: &Arc<ModEntry>, dupe_b: &Arc<ModEntry>) -> Flex<App> {
    let meta = metadata(&dupe_a.path);
    Flex::column()
      .with_child(Label::wrapped(format!("Version: {}", dupe_a.version)))
      .with_child(Label::wrapped(format!(
        "Path: {}",
        dupe_a.path.to_string_lossy()
      )))
      .with_child(Label::wrapped(format!(
        "Last modified: {}",
        if let Ok(Ok(time)) = meta.as_ref().map(|meta| meta.modified()) {
          DateTime::<Local>::from(time).format("%F:%R").to_string()
        } else {
          "Failed to retrieve last modified".to_string()
        }
      )))
      .with_child(Label::wrapped(format!(
        "Created at: {}",
        meta.and_then(|meta| meta.created()).map_or_else(
          |_| "Failed to retrieve creation time".to_string(),
          |time| { DateTime::<Local>::from(time).format("%F:%R").to_string() }
        )
      )))
      .with_child(Button::new("Keep").on_click({
        let id = dupe_a.id.clone();
        let path = dupe_b.path.clone();
        let dupe_a = dupe_a.clone();
        move |ctx, _, _| {
          ctx.submit_command(
            App::REMOVE_DUPLICATE_LOG_ENTRY
              .with(id.clone())
              .to(Target::Global),
          );
          ctx.submit_command(
            App::DELETE_AND_SUMBIT
              .with((path.clone(), dupe_a.clone()))
              .to(Target::Global),
          )
        }
      }))
      .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
      .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
  }

  pub fn build_progress_bars() -> impl Widget<App> {
    Modal::new("Downloads")
      .with_content(
        List::new(|| {
          Flex::column()
            .with_child(Label::wrapped_lens(lens!((i64, String, f64), 1)))
            .with_child(
              Label::wrapped_func(|data, _| {
                let start_time = Local.timestamp_opt(*data, 0).unwrap().format("%I:%M%p");

                format!("Started at: {}", start_time)
              })
              .lens(lens!((i64, String, f64), 0)),
            )
            .with_child(
              Flex::row()
                .with_flex_child(
                  ProgressBar::new()
                    .with_corner_radius(0.0)
                    .with_bar_brush(druid::Color::GREEN.into())
                    .expand_width()
                    .lens(lens!((i64, String, f64), 2)),
                  1.,
                )
                .with_child(
                  Either::new(
                    |fraction, _| *fraction < 1.0,
                    Spinner::new(),
                    Icon::new(*VERIFIED),
                  )
                  .lens(lens!((i64, String, f64), 2)),
                )
                .with_child(
                  Either::new(
                    |fraction, _| *fraction < 1.0,
                    Icon::new(*CLOSE).with_color(druid::Color::GRAY),
                    Icon::new(*CLOSE),
                  )
                  .lens(lens!((i64, String, f64), 2))
                  .controller(HoverController::default())
                  .on_click(|ctx, data, _| {
                    ctx.submit_command(App::REMOVE_DOWNLOAD_BAR.with(data.0))
                  })
                  .disabled_if(|data, _| data.2 < 1.0),
                ),
            )
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        })
        .lens(App::downloads)
        .boxed(),
      )
      .with_close()
      .build()
  }

  pub fn build_found_multiple(source: HybridPath, found_paths: Vec<PathBuf>) -> impl Widget<App> {
    let title = format!(
      "Found multiple mods in {}",
      match source {
        HybridPath::PathBuf(_) => "folder",
        HybridPath::Temp(_, _, _) => "archive",
      }
    );

    let mods = found_paths
      .iter()
      .filter_map(|path| ModEntry::from_file(path, ModMetadata::default()).ok())
      .map(|entry| (true, entry))
      .collect::<Vector<_>>();

    let modal = Modal::new(&title)
      .pipe(|mut modal| {
        for (idx, (_, mod_)) in mods.iter().enumerate() {
          modal = modal
            .with_content(
              Label::wrapped(format!("Found mod with ID: {}", mod_.id))
                .or_empty(|(data, _): &(bool, ModEntry), _| *data)
                .lens(lens::Index::new(idx))
                .boxed(),
            )
            .with_content(
              Flex::row()
                .with_flex_child(
                  Label::wrapped(format!("At path: {}", mod_.path.to_string_lossy()))
                    .expand_width(),
                  1.,
                )
                .with_child(
                  Button2::new(Label::new("Open path").with_text_size(14.)).on_click({
                    let path = mod_.path.clone();
                    move |_, _, _| {
                      let _ = opener::open(path.clone());
                    }
                  }),
                )
                .or_empty(|(data, _): &(bool, ModEntry), _| *data)
                .lens(lens::Index::new(idx))
                .boxed(),
            )
            .with_content(
              Button2::from_label("Install")
                .on_click({
                  let source = source.clone();
                  move |ctx, (show, entry): &mut (bool, ModEntry), _| {
                    *show = false;

                    let mut vec = Vector::new();
                    vec.push_back(entry.path.clone());
                    ctx.submit_command_global(
                      INSTALL_ALL.with(SingleUse::new((vec, source.clone()))),
                    )
                  }
                })
                .or_empty(|(data, _): &(bool, ModEntry), _| *data)
                .lens(lens::Index::new(idx))
                .boxed(),
            )
        }

        modal
      })
      .with_button("Install All", {
        let source = source.clone();
        move |ctx: &mut EventCtx, data: &mut Vector<(bool, ModEntry)>| {
          ctx.submit_command_global(
            INSTALL_ALL.with(SingleUse::new((
              data
                .iter()
                .filter_map(|(install, entry)| install.then(|| entry.path.clone()))
                .collect(),
              source,
            ))),
          )
        }
      })
      .with_close_label("Ignore All")
      .build();

    Scope::from_function(move |_| mods, DummyTransfer::default(), modal)
  }
}

pub enum SubwindowType {
  Log,
  Overwrite,
  Duplicate,
  Download,
}

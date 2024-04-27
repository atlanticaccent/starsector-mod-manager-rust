use std::{fs::File, io::Write as _, path::PathBuf, rc::Rc};

use base64::{decode, encode};
use druid::{
  commands, keyboard_types::Key, widget::Label, AppDelegate as Delegate, Command, DelegateCtx, Env,
  Event, Handled, KeyEvent, LensExt as _, SingleUse, Size, Target, Widget, WidgetExt as _,
  WindowDesc, WindowHandle, WindowId, WindowLevel,
};
use itertools::Itertools;
use rand::random;
use remove_dir_all::remove_dir_all;
use reqwest::Url;
use webview_shared::{
  InstallType, UserEvent, PROJECT, WEBVIEW_EVENT, WEBVIEW_INSTALL, WEBVIEW_OFFSET,
};
use webview_subsystem::init_webview;

use super::{
  installer::{self, DOWNLOAD_PROGRESS, DOWNLOAD_STARTED},
  mod_description,
  mod_list::{install::install_options::InstallOptions, ModList},
  overlays::Popup,
  settings::{self, Settings, SettingsCommand},
  tools,
  util::{get_latest_manager, get_starsector_version, GET_INSTALLED_STARSECTOR},
  App,
};

pub enum AppCommands {
  UpdateModDescription(String),
  PickFile(bool),
}

#[derive(Default)]
pub struct AppDelegate {
  pub root_id: Option<WindowId>,
  pub root_window: Option<WindowHandle>,
  pub mega_file: Option<(File, PathBuf)>,
  pub startup_popups: Vec<Popup>,

  // deprecated
  pub settings_id: Option<WindowId>,
  pub log_window: Option<WindowId>,
  pub overwrite_window: Option<WindowId>,
  pub duplicate_window: Option<WindowId>,
  pub download_window: Option<WindowId>,
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
        }
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
        data.settings.vmparams = tools::vmparams::VMParams::load(new_install_dir).ok();

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
          eprintln!("Failed to save settings")
        };
        data.mod_list.install_dir_available = true
      }
      return Handled::No;
    } else if let Some(entry) = cmd.get(ModList::AUTO_UPDATE) {
      ctx.submit_command(App::LOG_MESSAGE.with(format!("Begin auto-update of {}", entry.name)));
      data
        .runtime
        .spawn(installer::Payload::Download(entry.into()).install(
          ctx.get_external_handle(),
          data.settings.install_dir.clone().unwrap(),
          data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
        ));
    } else if let Some(()) = cmd.get(App::REFRESH) {
      if let Some(install_dir) = data.settings.install_dir.as_ref() {
        // data.mod_list.mods.clear();
        data.runtime.spawn(ModList::parse_mod_folder_async(
          install_dir.clone(),
          ctx.get_external_handle(),
        ));
      }

      return Handled::Yes;
    } else if let Some(res) = cmd.get(GET_INSTALLED_STARSECTOR) {
      App::mod_list
        .then(ModList::starsector_version)
        .put(data, res.as_ref().ok().cloned());
    } else if let Some(name) = cmd.get(App::LOG_SUCCESS) {
      data.log_message(&format!("Successfully installed {}", name));
      return Handled::Yes;
    } else if let Some(()) = cmd.get(App::CLEAR_LOG) {
      data.log.clear();

      return Handled::Yes;
    } else if let Some((name, err)) = cmd.get(App::LOG_ERROR) {
      data.log_message(&format!("Failed to install {}. Error: {}", name, err));
      return Handled::Yes;
    } else if let Some(message) = cmd.get(App::LOG_MESSAGE) {
      data.log_message(message);
      return Handled::Yes;
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
          installer::Payload::Initial(vec![path.into()])
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
    } else if let Some(entry) = cmd.get(App::CONFIRM_DELETE_MOD) {
      if remove_dir_all(&entry.path).is_ok() {
        data.mod_list.mods.remove(&entry.id);
        data.active = None;
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
          if data.settings.dirty
            && let Some(install_dir) = data.settings.install_dir.as_ref()
          {
            ctx.submit_command(
              Settings::SELECTOR.with(SettingsCommand::UpdateInstallDir(install_dir.clone())),
            );
          }
          let ext_ctx = ctx.get_external_handle();
          data.runtime.spawn(async move {
            let release = get_latest_manager().await;
            ext_ctx.submit_command(App::UPDATE_AVAILABLE, release, Target::Auto)
          });

          let mut delayed_popups = Vec::new();
          if !data
            .settings
            .install_dir
            .as_ref()
            .is_some_and(|p| p.exists())
          {
            delayed_popups.push(Popup::SelectInstall)
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
      }
      Event::MouseDown(ref mouse) => {
        ctx.submit_command(InstallOptions::DISMISS.with(mouse.window_pos))
      }
      _ => {}
    }

    Some(event)
  }
}

impl AppDelegate {
  pub fn with_popups(mut self, popups: Vec<Popup>) -> Self {
    self.startup_popups = popups;
    self
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
        _ => unimplemented!(),
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

  pub fn build_progress_bars() -> impl Widget<App> {
    /* Modal::new("Downloads")
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
    .build() */
    Label::new("foo")
  }
}

pub enum SubwindowType {
  Log,
  Overwrite,
  Duplicate,
  Download,
}

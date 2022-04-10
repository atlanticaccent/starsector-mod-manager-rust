use std::{cell::RefCell, fs::metadata, path::PathBuf, process::Child, rc::Rc, sync::Arc};

use chrono::{DateTime, Local};
use directories::ProjectDirs;
use druid::{
  commands,
  im::Vector,
  keyboard_types::Key,
  lens, theme,
  widget::{
    Axis, Button, Checkbox, Flex, Label, Maybe, Painter, Scope, ScopeTransfer, SizedBox, Tabs,
    TabsPolicy, TextBox, ViewSwitcher,
  },
  AppDelegate as Delegate, Command, Data, DelegateCtx, Env, Event, EventCtx, Handled, KeyEvent,
  Lens, LensExt, RenderContext, Selector, Target, Widget, WidgetExt, WidgetId, WindowDesc,
  WindowId, WindowLevel,
};
use druid_widget_nursery::{material_icons::Icon, Separator, WidgetExt as WidgetExtNursery};
use lazy_static::lazy_static;
use rand::random;
use remove_dir_all::remove_dir_all;
use reqwest::Url;
use strum::IntoEnumIterator;
use tap::{Pipe, Tap};
use tokio::runtime::Handle;

use crate::{
  patch::{
    split::Split,
    tabs_policy::{InitialTab, StaticTabsForked},
  },
  webview::{
    fork_into_webview, kill_server_thread, maximize_webview, InstallType, WEBVIEW_INSTALL,
    WEBVIEW_SHUTDOWN,
  },
};

use self::{
  installer::{ChannelMessage, HybridPath, StringOrPath},
  mod_description::ModDescription,
  mod_entry::ModEntry,
  mod_list::{EnabledMods, Filters, ModList},
  modal::Modal,
  settings::{Settings, SettingsCommand},
  util::{
    get_latest_manager, get_quoted_version, get_starsector_version, h2, h3, icons::*,
    make_column_pair, LabelExt, Release, GET_INSTALLED_STARSECTOR,
  },
  controllers::{InstallController, ModListController, AppController},
};

mod controllers;
pub mod installer;
mod mod_description;
mod mod_entry;
mod mod_list;
pub mod modal;
mod settings;
mod updater;
#[path = "./util.rs"]
pub mod util;

const TAG: &str = env!("CARGO_PKG_VERSION");

lazy_static! {
  pub static ref PROJECT: ProjectDirs =
    ProjectDirs::from("org", "laird", "Starsector Mod Manager").expect("Get project dirs");
}

#[derive(Clone, Data, Lens)]
pub struct App {
  init: bool,
  settings: settings::Settings,
  mod_list: mod_list::ModList,
  active: Option<Arc<ModEntry>>,
  #[data(ignore)]
  runtime: Handle,
  #[data(ignore)]
  widget_id: WidgetId,
  #[data(same_fn = "PartialEq::eq")]
  log: Vec<String>,
  overwrite_log: Vector<Rc<(StringOrPath, HybridPath, Arc<ModEntry>)>>,
  duplicate_log: Vector<(Arc<ModEntry>, Arc<ModEntry>)>,
  webview: Option<Rc<RefCell<Child>>>,
}

impl App {
  const SELECTOR: Selector<AppCommands> = Selector::new("app.update.commands");
  const OPEN_FILE: Selector<Option<Vec<PathBuf>>> = Selector::new("app.open.multiple");
  const OPEN_FOLDER: Selector<Option<PathBuf>> = Selector::new("app.open.folder");
  pub const ENABLE: Selector<()> = Selector::new("app.enable");
  const DUMB_UNIVERSAL_ESCAPE: Selector<()> = Selector::new("app.universal_escape");
  const REFRESH: Selector<()> = Selector::new("app.mod_list.refresh");
  const DISABLE: Selector<()> = Selector::new("app.disable");
  const UPDATE_AVAILABLE: Selector<Result<Release, String>> = Selector::new("app.update.available");
  const SELF_UPDATE: Selector<()> = Selector::new("app.update.perform");
  const RESTART: Selector<PathBuf> = Selector::new("app.update.restart");
  const LOG_SUCCESS: Selector<String> = Selector::new("app.mod.install.success");
  const CLEAR_LOG: Selector = Selector::new("app.install.clear_log");
  const LOG_ERROR: Selector<(String, String)> = Selector::new("app.mod.install.fail");
  const LOG_MESSAGE: Selector<String> = Selector::new("app.mod.install.start");
  const LOG_OVERWRITE: Selector<(StringOrPath, HybridPath, Arc<ModEntry>)> =
    Selector::new("app.mod.install.overwrite");
  const CLEAR_OVERWRITE_LOG: Selector<bool> = Selector::new("app.install.clear_overwrite_log");
  const REMOVE_OVERWRITE_LOG_ENTRY: Selector<StringOrPath> =
    Selector::new("app.install.overwrite.decline");
  const DELETE_AND_SUMBIT: Selector<(PathBuf, Arc<ModEntry>)> =
    Selector::new("app.mod.duplicate.resolve");
  const REMOVE_DUPLICATE_LOG_ENTRY: Selector<String> =
    Selector::new("app.mod.duplicate.remove_log");
  const CLEAR_DUPLICATE_LOG: Selector = Selector::new("app.mod.duplicate.ignore_all");
  pub const OPEN_WEBVIEW: Selector<Option<String>> = Selector::new("app.webview.open");

  pub fn new(handle: Handle) -> Self {
    App {
      init: false,
      settings: settings::Settings::load()
        .map(|mut settings| {
          if settings.vmparams_enabled {
            if let Some(path) = settings.install_dir.clone() {
              settings.vmparams = settings::vmparams::VMParams::load(path).ok();
            }
          }
          if let Some(install_dir) = settings.install_dir.clone() {
            settings.install_dir_buf = install_dir.to_string_lossy().to_string()
          }
          settings
        })
        .unwrap_or_else(|_| settings::Settings::default()),
      mod_list: mod_list::ModList::new(),
      active: None,
      runtime: handle,
      widget_id: WidgetId::reserved(0),
      log: Vec::new(),
      overwrite_log: Vector::new(),
      duplicate_log: Vector::new(),
      webview: None,
    }
  }

  pub fn ui_builder() -> impl Widget<Self> {
    let button_painter = || {
      Painter::new(|ctx, _, env| {
        let is_active = ctx.is_active() && !ctx.is_disabled();
        let is_hot = ctx.is_hot();
        let size = ctx.size();
        let stroke_width = env.get(theme::BUTTON_BORDER_WIDTH);

        let rounded_rect = size
          .to_rect()
          .inset(-stroke_width / 2.0)
          .to_rounded_rect(env.get(theme::BUTTON_BORDER_RADIUS));

        let bg_gradient = if ctx.is_disabled() {
          env.get(theme::DISABLED_BUTTON_DARK)
        } else if is_active {
          env.get(theme::BUTTON_DARK)
        } else {
          env.get(theme::BUTTON_LIGHT)
        };

        let border_color = if is_hot && !ctx.is_disabled() {
          env.get(theme::BORDER_LIGHT)
        } else {
          env.get(theme::BORDER_DARK)
        };

        ctx.stroke(rounded_rect, &border_color, stroke_width);

        ctx.fill(rounded_rect, &bg_gradient);
      })
    };
    let settings = Flex::row()
      .with_child(
        Flex::row()
          .with_child(Label::new("Settings").with_text_size(18.))
          .with_spacer(5.)
          .with_child(Icon::new(SETTINGS))
          .padding((8., 4.))
          .background(button_painter())
          .on_click(|event_ctx, _, _| {
            event_ctx.submit_command(App::SELECTOR.with(AppCommands::OpenSettings))
          }),
      )
      .expand_width();
    let refresh = Flex::row()
      .with_child(
        Flex::row()
          .with_child(Label::new("Refresh").with_text_size(18.))
          .with_spacer(5.)
          .with_child(Icon::new(SYNC))
          .padding((8., 4.))
          .background(button_painter())
          .on_click(|event_ctx, _, _| event_ctx.submit_command(App::REFRESH)),
      )
      .expand_width();
    let install_dir_browser =
      Settings::install_dir_browser_builder(Axis::Vertical).lens(App::settings);
    let install_mod_button = Flex::row()
      .with_child(Label::new("Install Mod(s)").with_text_size(18.))
      .with_spacer(5.)
      .with_child(Icon::new(INSTALL_DESKTOP))
      .padding((8., 4.))
      .background(button_painter())
      .on_click(|_, _, _| {})
      .controller(InstallController)
      .on_command(App::OPEN_FILE, |ctx, payload, data| {
        if let Some(targets) = payload {
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
            installer::Payload::Initial(targets.iter().map(|f| f.to_path_buf()).collect()).install(
              ctx.get_external_handle(),
              data.settings.install_dir.clone().unwrap(),
              data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
            ),
          );
        }
      })
      .on_command(App::OPEN_FOLDER, |ctx, payload, data| {
        if let Some(target) = payload {
          ctx.submit_command(App::LOG_MESSAGE.with(format!(
            "Installing {}",
            target.file_name().map_or_else(
              || String::from("unknown"),
              |f| f.to_string_lossy().into_owned(),
            )
          )));
          data
            .runtime
            .spawn(installer::Payload::Initial(vec![target.clone()]).install(
              ctx.get_external_handle(),
              data.settings.install_dir.clone().unwrap(),
              data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
            ));
        }
      });
    let browse_index_button = Flex::row()
      .with_child(
        Flex::row()
          .with_child(Label::new("Open Mod Browser").with_text_size(18.))
          .with_spacer(5.)
          .with_child(Icon::new(OPEN_IN_BROWSER))
          .padding((8., 4.))
          .background(button_painter())
          .on_click(|event_ctx, _, _| event_ctx.submit_command(App::OPEN_WEBVIEW.with(None))),
      )
      .expand_width();
    let mod_list = mod_list::ModList::ui_builder()
      .lens(App::mod_list)
      .on_change(|_ctx, _old, data, _env| {
        if let Some(install_dir) = &data.settings.install_dir {
          let enabled: Vec<Arc<ModEntry>> = data
            .mod_list
            .mods
            .iter()
            .filter_map(|(_, v)| v.enabled.then(|| v.clone()))
            .collect();

          if let Err(err) = EnabledMods::from(enabled).save(install_dir) {
            eprintln!("{:?}", err)
          };
        }
      })
      .expand()
      .controller(ModListController);
    let mod_description = ViewSwitcher::new(
      |data: &App, _| (data.active.clone(), data.webview.is_some()),
      |(active, enabled), _, _| {
        if let Some(active) = active {
          let enabled = *enabled;
          ModDescription::ui_builder()
            .lens(lens::Constant(active.clone()))
            .disabled_if(move |_, _| enabled)
            .boxed()
        } else {
          Box::new(ModDescription::empty_builder().lens(lens::Unit))
        }
      },
    );
    let tool_panel = Flex::column()
      .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
      .with_child(h2("Search"))
      .with_child(
        TextBox::new()
          .on_change(|ctx, _, _, _| {
            ctx.submit_command(ModList::SEARCH_UPDATE);
          })
          .lens(App::mod_list.then(ModList::search_text))
          .expand_width(),
      )
      .with_default_spacer()
      .with_child(h2("Toggles"))
      .with_child(
        Button::new("Enable All")
          .disabled_if(|data: &App, _| data.mod_list.mods.values().all(|e| e.enabled))
          .on_click(|_, data: &mut App, _| {
            if let Some(install_dir) = data.settings.install_dir.as_ref() {
              let id = data.active.as_ref().map(|e| e.id.clone());
              data.active = None;
              let mut enabled: Vec<String> = Vec::new();
              data.mod_list.mods = data
                .mod_list
                .mods
                .drain_filter(|_, _| true)
                .map(|(id, mut entry)| {
                  (Arc::make_mut(&mut entry)).enabled = true;
                  enabled.push(id.clone());
                  (id, entry)
                })
                .collect();
              data.active = id
                .as_ref()
                .and_then(|id| data.mod_list.mods.get(id).cloned());
              if let Err(err) = EnabledMods::from(enabled).save(install_dir) {
                eprintln!("{:?}", err)
              }
            }
          })
          .expand_width(),
      )
      .with_spacer(5.)
      .with_child(
        Button::new("Disable All")
          .disabled_if(|data: &App, _| data.mod_list.mods.values().all(|e| !e.enabled))
          .on_click(|_, data: &mut App, _| {
            if let Some(install_dir) = data.settings.install_dir.as_ref() {
              let id = data.active.as_ref().map(|e| e.id.clone());
              data.active = None;
              data.mod_list.mods = data
                .mod_list
                .mods
                .drain_filter(|_, _| true)
                .map(|(id, mut entry)| {
                  (Arc::make_mut(&mut entry)).enabled = false;
                  (id, entry)
                })
                .collect();
              data.active = id
                .as_ref()
                .and_then(|id| data.mod_list.mods.get(id).cloned());
              if let Err(err) = EnabledMods::empty().save(install_dir) {
                eprintln!("{:?}", err)
              }
            }
          })
          .expand_width(),
      )
      .with_default_spacer()
      .with_child(h2("Filters"))
      .tap_mut(|panel| {
        for filter in Filters::iter() {
          match filter {
            Filters::Enabled => panel.add_child(h3("Status")),
            Filters::Unimplemented => panel.add_child(h3("Version Checker")),
            Filters::AutoUpdateAvailable => panel.add_child(h3("Auto Update Support")),
            _ => {}
          };
          panel.add_child(
            Scope::from_function(
              |state: bool| state,
              IndyToggleState { state: true },
              Checkbox::from_label(Label::wrapped(&filter.to_string())).on_change(
                move |ctx, _, new, _| {
                  ctx.submit_command(ModList::FILTER_UPDATE.with((filter, !*new)))
                },
              ),
            )
            .lens(lens::Constant(true)),
          )
        }
      })
      .padding(20.);
    let launch_panel = Flex::column()
      .with_child(make_column_pair(
        h2("Starsector Version:"),
        Maybe::new(
          || Label::wrapped_func(|v: &String, _| v.clone()),
          || Label::new("Unknown"),
        )
        .lens(
          App::mod_list
            .then(ModList::starsector_version)
            .map(|v| v.as_ref().and_then(get_quoted_version), |_, _| {}),
        ),
      ))
      .with_default_spacer()
      .with_child(install_dir_browser)
      .with_default_spacer()
      .with_child(ViewSwitcher::new(
        |data: &App, _| data.settings.install_dir.is_some(),
        move |has_dir, _, _| {
          if *has_dir {
            Box::new(
              Flex::row()
                .with_flex_child(h2("Launch Starsector").expand_width(), 2.)
                .with_flex_child(Icon::new(PLAY_ARROW).expand_width(), 1.)
                .padding((8., 4.))
                .background(button_painter())
                .on_click(|ctx, data: &mut App, _| {
                  if let Some(install_dir) = data.settings.install_dir.clone() {
                    ctx.submit_command(App::DISABLE);
                    let ext_ctx = ctx.get_external_handle();
                    let experimental_launch = data.settings.experimental_launch;
                    let resolution = data.settings.experimental_resolution;
                    data.runtime.spawn(async move {
                      if let Err(err) =
                        App::launch_starsector(install_dir, experimental_launch, resolution).await
                      {
                        dbg!(err);
                      };
                      ext_ctx.submit_command(App::ENABLE, (), Target::Auto)
                    });
                  }
                })
                .expand_width(),
            )
          } else {
            Box::new(SizedBox::empty())
          }
        },
      ))
      .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
      .expand()
      .padding(20.);
    let side_panel = Tabs::for_policy(
      StaticTabsForked::build(vec![
        InitialTab::new("Launch", launch_panel),
        InitialTab::new("Tools & Filters", tool_panel),
      ])
      .set_label_height(40.0),
    );

    Flex::column()
      .with_child(
        Flex::row()
          .with_child(settings)
          .with_spacer(10.)
          .with_child(install_mod_button)
          .with_spacer(10.)
          .with_child(browse_index_button)
          .with_spacer(10.)
          .with_child(refresh)
          .with_spacer(10.)
          .with_child(
            ViewSwitcher::new(
              |len: &usize, _| *len,
              |len, _, _| Box::new(h3(&format!("Installed: {}", len))),
            )
            .lens(
              App::mod_list
                .then(ModList::mods)
                .map(|data| data.len(), |_, _| {}),
            ),
          )
          .with_spacer(10.)
          .with_child(
            ViewSwitcher::new(
              |len: &usize, _| *len,
              |len, _, _| Box::new(h3(&format!("Active: {}", len))),
            )
            .lens(App::mod_list.then(ModList::mods).map(
              |data| data.values().filter(|e| e.enabled).count(),
              |_, _| {},
            )),
          )
          .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
          .expand_width(),
      )
      .with_spacer(20.)
      .with_flex_child(
        Split::columns(mod_list, side_panel)
          .split_point(0.8)
          .draggable(true)
          .expand_height(),
        2.0,
      )
      .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
      .with_flex_child(mod_description, 1.0)
      .must_fill_main_axis(true)
      .controller(AppController)
      .with_id(WidgetId::reserved(0))
  }

  async fn launch_starsector(
    install_dir: PathBuf,
    experimental_launch: bool,
    resolution: (u32, u32),
  ) -> Result<(), String> {
    use tokio::fs::read_to_string;
    use tokio::process::Command;

    lazy_static! {
      static ref JAVA_REGEX: regex::Regex = regex::Regex::new(r"java\.exe").expect("compile regex");
    }

    let child = if experimental_launch {
      // let mut args_raw = String::from(r"java.exe -XX:CompilerThreadPriority=1 -XX:+CompilerThreadHintNoPreempt -XX:+DisableExplicitGC -XX:+UnlockExperimentalVMOptions -XX:+AggressiveOpts -XX:+TieredCompilation -XX:+UseG1GC -XX:InitialHeapSize=2048m -XX:MaxMetaspaceSize=2048m -XX:MaxNewSize=2048m -XX:+ParallelRefProcEnabled -XX:G1NewSizePercent=5 -XX:G1MaxNewSizePercent=10 -XX:G1ReservePercent=5 -XX:G1MixedGCLiveThresholdPercent=70 -XX:InitiatingHeapOccupancyPercent=90 -XX:G1HeapWastePercent=5 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=2M -XX:+UseStringDeduplication -Djava.library.path=native\windows -Xms1536m -Xmx1536m -Xss2048k -classpath janino.jar;commons-compiler.jar;commons-compiler-jdk.jar;starfarer.api.jar;starfarer_obf.jar;jogg-0.0.7.jar;jorbis-0.0.15.jar;json.jar;lwjgl.jar;jinput.jar;log4j-1.2.9.jar;lwjgl_util.jar;fs.sound_obf.jar;fs.common_obf.jar;xstream-1.4.10.jar -Dcom.fs.starfarer.settings.paths.saves=..\\saves -Dcom.fs.starfarer.settings.paths.screenshots=..\\screenshots -Dcom.fs.starfarer.settings.paths.mods=..\\mods -Dcom.fs.starfarer.settings.paths.logs=. com.fs.starfarer.StarfarerLauncher");
      let mut args_raw = read_to_string(install_dir.join("vmparams"))
        .await
        .map_err(|err| err.to_string())?;
      args_raw = JAVA_REGEX.replace(&args_raw, "").to_string();
      let args: Vec<&str> = args_raw.split_ascii_whitespace().collect();

      Command::new(install_dir.join("jre").join("bin").join("java.exe"))
        .current_dir(install_dir.join("starsector-core"))
        .args([
          "-DlaunchDirect=true",
          &format!("-DstartRes={}x{}", resolution.0, resolution.1),
          "-DstartFS=false",
          "-DstartSound=true",
        ])
        .args(args)
        .spawn()
        .expect("Execute Starsector")
    } else {
      Command::new(install_dir.join("starsector.exe"))
        .current_dir(install_dir)
        .spawn()
        .expect("Execute Starsector")
    };

    child
      .wait_with_output()
      .await
      .map_or_else(|err| Err(err.to_string()), |_| Ok(()))
  }

  fn log_message(&mut self, message: &str) {
    self
      .log
      .push(format!("[{}] {}", Local::now().format("%H:%M:%S"), message))
  }

  fn push_overwrite(&mut self, message: (StringOrPath, HybridPath, Arc<ModEntry>)) {
    if !self.overwrite_log.iter().any(|val| val.0 == message.0) {
      self.overwrite_log.push_back(Rc::new(message))
    }
  }

  fn push_duplicate(&mut self, duplicates: &(Arc<ModEntry>, Arc<ModEntry>)) {
    self.duplicate_log.push_back(duplicates.clone())
  }
}

enum AppCommands {
  OpenSettings,
  UpdateModDescription(Arc<ModEntry>),
}

#[derive(Default)]
pub struct AppDelegate {
  settings_id: Option<WindowId>,
  root_id: Option<WindowId>,
  log_window: Option<WindowId>,
  fail_window: Option<WindowId>,
  overwrite_window: Option<WindowId>,
  duplicate_window: Option<WindowId>,
}

impl Delegate<App> for AppDelegate {
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
        AppCommands::OpenSettings => {
          let install_dir = lens!(App, settings)
            .then(lens!(settings::Settings, install_dir))
            .get(data);
          lens!(App, settings)
            .then(lens!(settings::Settings, install_dir_buf))
            .put(
              data,
              install_dir.map_or_else(|| "".to_string(), |p| p.to_string_lossy().to_string()),
            );

          let settings_window = WindowDesc::new(
            settings::Settings::ui_builder()
              .lens(App::settings)
              .on_change(|_, _old, data, _| {
                if let Err(err) = data.settings.save() {
                  eprintln!("{:?}", err)
                }
              }),
          )
          .window_size((800., 400.))
          .show_titlebar(false);

          self.settings_id = Some(settings_window.id);

          ctx.new_window(settings_window);
          return Handled::Yes;
        }
        AppCommands::UpdateModDescription(desc) => {
          data.active = Some(desc.clone());

          return Handled::Yes;
        }
      }
    } else if let Some(SettingsCommand::UpdateInstallDir(new_install_dir)) =
      cmd.get(settings::Settings::SELECTOR)
    {
      if data.settings.install_dir != Some(new_install_dir.clone()) || data.settings.dirty {
        data.settings.dirty = false;
        data.settings.install_dir_buf = new_install_dir.to_string_lossy().to_string();
        data.settings.install_dir = Some(new_install_dir.clone());

        if data.settings.save().is_err() {
          eprintln!("Failed to save settings")
        };

        data.mod_list.mods.clear();
        data.runtime.spawn(get_starsector_version(
          ctx.get_external_handle(),
          new_install_dir.clone(),
        ));
        data.runtime.spawn(ModList::parse_mod_folder(
          ctx.get_external_handle(),
          Some(new_install_dir.clone()),
        ));
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
        data.mod_list.mods.clear();
        data.runtime.spawn(ModList::parse_mod_folder(
          ctx.get_external_handle(),
          Some(install_dir.clone()),
        ));
      }
    } else if let Some(res) = cmd.get(GET_INSTALLED_STARSECTOR) {
      App::mod_list
        .then(ModList::starsector_version)
        .put(data, res.as_ref().ok().cloned());
    } else if let Some(entry) = cmd
      .get(ModEntry::REPLACE)
      .or_else(|| cmd.get(ModList::SUBMIT_ENTRY))
      .or_else(|| {
        cmd.get(installer::INSTALL).and_then(|m| {
          if let ChannelMessage::Success(e) = m {
            Some(e)
          } else {
            None
          }
        })
      })
    {
      if Some(&entry.id) == data.active.as_ref().map(|e| &e.id) {
        data.active = Some(entry.clone())
      }
    } else if let Some(name) = cmd.get(App::LOG_SUCCESS) {
      data.log_message(&format!("Successfully installed {}", name));
      self.display_log_if_closed(ctx);

      return Handled::Yes;
    } else if let Some(()) = cmd.get(App::CLEAR_LOG) {
      data.log.clear();

      return Handled::Yes;
    } else if let Some((name, err)) = cmd.get(App::LOG_ERROR) {
      data.log_message(&format!("Failed to install {}. Error: {}", name, err));
      self.display_log_if_closed(ctx);

      return Handled::Yes;
    } else if let Some(message) = cmd.get(App::LOG_MESSAGE) {
      data.log_message(message);
      self.display_log_if_closed(ctx);

      return Handled::Yes;
    } else if let Some(message) = cmd.get(App::LOG_OVERWRITE) {
      data.push_overwrite(message.clone());
      self.display_overwrite_if_closed(ctx);

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
      self.display_duplicate_if_closed(ctx);

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
            util::get_master_version(ext_ctx, version_meta).await;
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
    } else if let Some(()) = cmd.get(WEBVIEW_SHUTDOWN) {
      data.webview = None;

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
                .unwrap_or(uri.clone())
                .to_string();
              ext_ctx
                .submit_command(
                  App::LOG_MESSAGE,
                  format!("Installing {}", &file_name),
                  Target::Auto,
                )
                .expect("Send install start");
              let download = installer::download(uri).await.expect("Download archive");
              let download_dir = PROJECT.cache_dir().to_path_buf();
              let mut persist_path = download_dir.join(&file_name);
              if persist_path.exists() {
                persist_path = download_dir.join(format!("{}({})", file_name, random::<u8>()))
              }
              download.persist(&persist_path).expect("Persist download");

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
    } else if let Some(url) = cmd.get(App::OPEN_WEBVIEW) {
      ctx.submit_command(App::DISABLE);
      let child = fork_into_webview(&data.runtime, ctx.get_external_handle(), url.clone());

      data.webview = Some(Rc::new(RefCell::new(child)))
    } else if let Some(url) = cmd.get(mod_description::OPEN_IN_WEBVIEW) {
      if data.settings.open_forum_link_in_webview {
        ctx.submit_command(App::OPEN_WEBVIEW.with(Some(url.clone())));
      } else {
        let _ = opener::open(url);
      }
    }

    Handled::No
  }

  fn window_removed(&mut self, id: WindowId, data: &mut App, _env: &Env, ctx: &mut DelegateCtx) {
    match Some(id) {
      a if a == self.settings_id => self.settings_id = None,
      a if a == self.log_window => self.log_window = None,
      a if a == self.fail_window => self.fail_window = None,
      a if a == self.overwrite_window => {
        data.overwrite_log.clear();
        self.overwrite_window = None;
      }
      a if a == self.root_id => {
        println!("quitting");
        if let Some(child) = &data.webview {
          kill_server_thread();
          let _ = child.borrow_mut().kill();
          data.webview = None;
        }
        let _ = std::fs::remove_dir_all(PROJECT.cache_dir());
        ctx.submit_command(commands::QUIT_APP)
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
    if let druid::Event::WindowConnected = event {
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
    } else if let Event::KeyDown(KeyEvent {
      key: Key::Escape, ..
    }) = event
    {
      ctx.submit_command(App::DUMB_UNIVERSAL_ESCAPE)
    }

    Some(event)
  }
}

impl AppDelegate {
  fn build_log_window() -> impl Widget<App> {
    ViewSwitcher::new(
      |data: &App, _| data.log.len(),
      |_, data: &App, _| {
        let mut modal = Modal::new("Log").with_content("");

        for val in data.log.iter() {
          modal = modal.with_content(val.as_str())
        }

        modal.with_button("Close", App::CLEAR_LOG).build().boxed()
      },
    )
  }

  fn display_log_if_closed(&mut self, ctx: &mut DelegateCtx) {
    if self.log_window.is_none() {
      let modal = AppDelegate::build_log_window();

      let log_window = WindowDesc::new(modal)
        .window_size((500., 400.))
        .show_titlebar(false)
        .set_level(WindowLevel::AppWindow);

      self.log_window = Some(log_window.id);

      ctx.new_window(log_window);
    }
  }

  fn build_overwrite_window() -> impl Widget<App> {
    ViewSwitcher::new(
      |data: &App, _| data.overwrite_log.len(),
      |_, data: &App, _| {
        let mut modal = Modal::new("Overwrite?");

        for val in data.overwrite_log.iter() {
          let (conflict, to_install, entry) = val.as_ref();
          modal = modal
            .with_content(match conflict {
              StringOrPath::String(id) => format!("A mod with ID {} alread exists.", id),
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
                    maximize_webview();
                  }
                }))
                .with_child(Button::new("Cancel").on_click({
                  let conflict = conflict.clone();
                  move |ctx, _, _| {
                    ctx.submit_command(App::REMOVE_OVERWRITE_LOG_ENTRY.with(conflict.clone()));
                    maximize_webview();
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

  fn display_overwrite_if_closed(&mut self, ctx: &mut DelegateCtx) {
    if self.overwrite_window.is_none() {
      let modal = Self::build_overwrite_window();

      let overwrite_window = WindowDesc::new(modal)
        .window_size((500., 400.))
        .show_titlebar(false)
        .set_level(WindowLevel::AppWindow);

      self.overwrite_window = Some(overwrite_window.id);

      ctx.new_window(overwrite_window);
    }
  }

  fn build_duplicate_window() -> impl Widget<App> {
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

  fn display_duplicate_if_closed(&mut self, ctx: &mut DelegateCtx) {
    if self.duplicate_window.is_none() {
      let modal = Self::build_duplicate_window();

      let duplicate_window = WindowDesc::new(modal)
        .window_size((500., 400.))
        .show_titlebar(false)
        .set_level(WindowLevel::AppWindow);

      self.duplicate_window = Some(duplicate_window.id);

      ctx.new_window(duplicate_window);
    }
  }

  fn make_dupe_col(dupe_a: &Arc<ModEntry>, dupe_b: &Arc<ModEntry>) -> Flex<App> {
    let meta = metadata(&dupe_a.path);
    Flex::column()
      .with_child(Label::wrapped(&format!("Version: {}", dupe_a.version)))
      .with_child(Label::wrapped(&format!(
        "Path: {}",
        dupe_a.path.to_string_lossy()
      )))
      .with_child(Label::wrapped(&format!(
        "Last modified: {}",
        if let Ok(Ok(time)) = meta.as_ref().map(|meta| meta.modified()) {
          DateTime::<Local>::from(time).format("%F:%R").to_string()
        } else {
          "Failed to retrieve last modified".to_string()
        }
      )))
      .with_child(Label::wrapped(&format!(
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
}

#[derive(Clone, Data, Lens)]
struct IndyToggleState {
  state: bool,
}

impl ScopeTransfer for IndyToggleState {
  type In = bool;
  type State = bool;

  fn read_input(&self, _: &mut Self::State, _: &Self::In) {}

  fn write_back_input(&self, _: &Self::State, _: &mut Self::In) {}
}

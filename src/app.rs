use std::{path::PathBuf, process, sync::Arc};

use druid::{
  commands,
  keyboard_types::Key,
  lens, theme,
  widget::{
    Axis, Button, Checkbox, Controller, Flex, Label, Maybe, Painter, Scope, ScopeTransfer, Scroll,
    SizedBox, Tabs, TabsPolicy, TextBox, ViewSwitcher,
  },
  AppDelegate as Delegate, Command, Data, DelegateCtx, Env, Event, EventCtx, Handled, KeyEvent,
  Lens, LensExt, Menu, MenuItem, RenderContext, Selector, Target, Widget, WidgetExt, WidgetId,
  WindowDesc, WindowId,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as WidgetExtNursery};
use lazy_static::lazy_static;
use rfd::FileDialog;
use self_update::version::bump_is_greater;
use strum::IntoEnumIterator;
use tap::Tap;
use tokio::runtime::Handle;
use chrono::Local;

use crate::patch::{
  split::Split,
  tabs_policy::{InitialTab, StaticTabsForked},
};

use self::{
  installer::{ChannelMessage, StringOrPath},
  mod_description::ModDescription,
  mod_entry::ModEntry,
  mod_list::{EnabledMods, Filters, ModList},
  modal::Modal,
  settings::{Settings, SettingsCommand},
  updater::{open_in_browser, self_update, support_self_update},
  util::{
    get_latest_manager, get_master_version, get_quoted_version, get_starsector_version, h2, h3,
    icons::*, make_column_pair, LabelExt, Release, GET_INSTALLED_STARSECTOR,
  },
};

mod installer;
mod mod_description;
mod mod_entry;
mod mod_list;
pub mod modal;
mod settings;
mod updater;
#[path = "./util.rs"]
pub mod util;

const TAG: &str = env!("CARGO_PKG_VERSION");

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
}

impl App {
  const SELECTOR: Selector<AppCommands> = Selector::new("app.update.commands");
  const OPEN_FILE: Selector<Option<Vec<PathBuf>>> = Selector::new("app.open.multiple");
  const OPEN_FOLDER: Selector<Option<PathBuf>> = Selector::new("app.open.folder");
  const ENABLE: Selector<()> = Selector::new("app.enable");
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
            installer::Payload::Initial(targets.iter().map(|f| f.to_path_buf()).collect())
              .install(
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
          data.runtime.spawn(
            installer::Payload::Initial(vec![target.clone()]).install(
              ctx.get_external_handle(),
              data.settings.install_dir.clone().unwrap(),
              data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
            ),
          );
        }
      });
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
      |active: &Option<Arc<ModEntry>>, _| active.clone(),
      |active, _, _| {
        if let Some(active) = active {
          Box::new(ModDescription::ui_builder().lens(lens::Constant(active.clone())))
        } else {
          Box::new(ModDescription::empty_builder().lens(lens::Unit))
        }
      },
    )
    .lens(App::active);
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
    self.log.push(format!("[{}] {}", Local::now().format("%H:%M:%S"), message))
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
    }

    Handled::No
  }

  fn window_removed(&mut self, id: WindowId, _data: &mut App, _env: &Env, ctx: &mut DelegateCtx) {
    match Some(id) {
      a @ _ if a == self.settings_id => self.settings_id = None,
      a @ _ if a == self.log_window => self.log_window = None,
      a @ _ if a == self.fail_window => self.fail_window = None,
      a @ _ if a == self.root_id => ctx.submit_command(commands::QUIT_APP),
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
    Modal::new("Log")
      .with_content("")
      .with_content(
        Scroll::new(ViewSwitcher::new(
          |data: &App, _| data.log.len(),
          |_, data: &App, _| {
            Flex::column()
              .tap_mut(|flex| {
                for val in data.log.iter() {
                  flex.add_child(Label::wrapped(val))
                }
              })
              .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
              .boxed()
          },
        ))
        .vertical()
        .boxed(),
      )
      .with_button("Close", App::CLEAR_LOG)
      .build()
  }

  fn display_log_if_closed(&mut self, ctx: &mut DelegateCtx) {
    if self.log_window.is_none() {
      let modal = AppDelegate::build_log_window();

      let log_window = WindowDesc::new(modal)
        .window_size((500., 400.))
        .show_titlebar(false);

      self.log_window = Some(log_window.id);

      ctx.new_window(log_window);
    }
  }
}
struct InstallController;

impl<W: Widget<App>> Controller<App, W> for InstallController {
  fn event(
    &mut self,
    child: &mut W,
    ctx: &mut EventCtx,
    event: &Event,
    data: &mut App,
    env: &druid::Env,
  ) {
    match event {
      Event::MouseDown(mouse_event) => {
        if mouse_event.button == druid::MouseButton::Left {
          ctx.set_active(true);
          ctx.request_paint();
        }
      }
      Event::MouseUp(mouse_event) => {
        if ctx.is_active() && mouse_event.button == druid::MouseButton::Left {
          ctx.set_active(false);
          if ctx.is_hot() {
            let ext_ctx = ctx.get_external_handle();
            let menu: Menu<App> = Menu::empty()
              .entry(MenuItem::new("From Archive(s)").on_activate(
                move |_ctx, data: &mut App, _| {
                  let ext_ctx = ext_ctx.clone();
                  data.runtime.spawn_blocking(move || {
                    let res = FileDialog::new()
                      .add_filter(
                        "Archives",
                        &["zip", "7z", "7zip", "rar", "rar4", "rar5", "tar"],
                      )
                      .pick_files();

                    ext_ctx.submit_command(App::OPEN_FILE, res, Target::Auto)
                  });
                },
              ))
              .entry(MenuItem::new("From Folder").on_activate({
                let ext_ctx = ctx.get_external_handle();
                move |_ctx, data: &mut App, _| {
                  data.runtime.spawn_blocking({
                    let ext_ctx = ext_ctx.clone();
                    move || {
                      let res = FileDialog::new().pick_folder();

                      ext_ctx.submit_command(App::OPEN_FOLDER, res, Target::Auto)
                    }
                  });
                }
              }));

            ctx.show_context_menu::<App>(menu, ctx.to_window(mouse_event.pos))
          }
          ctx.request_paint();
        }
      }
      _ => {}
    }

    child.event(ctx, event, data, env);
  }
}

struct ModListController;

impl<W: Widget<App>> Controller<App, W> for ModListController {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut App, env: &Env) {
    if let Event::Command(cmd) = event {
      if let Some((conflict, install_to, entry)) = cmd.get(ModList::OVERWRITE) {
        if let Some(install_dir) = &data.settings.install_dir {
          ctx.submit_command(App::LOG_MESSAGE.with(format!("Resuming install for {}", entry.name)));
          data.runtime.spawn(
            installer::Payload::Resumed(entry.clone(), install_to.clone(), conflict.clone())
              .install(
                ctx.get_external_handle(),
                install_dir.clone(),
                data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
              ),
          );
        }
        ctx.is_handled();
      } else if let Some(payload) = cmd.get(installer::INSTALL) {
        match payload {
          ChannelMessage::Success(entry) => {
            let mut entry = entry.clone();
            let existing = data.mod_list.mods.get(&entry.id);
            if let Some(remote_version_checker) = existing.and_then(|e| e.remote_version.clone()) {
              let mut mut_entry = Arc::make_mut(&mut entry);
              mut_entry.remote_version = Some(remote_version_checker);
              mut_entry.update_status = existing.and_then(|e| e.update_status.clone());
            } else if let Some(version_checker) = entry.version_checker.clone() {
              data.runtime.spawn(get_master_version(
                ctx.get_external_handle(),
                version_checker,
              ));
            }
            data.mod_list.mods.insert(entry.id.clone(), entry.clone());
            ctx.children_changed();
            ctx.submit_command(App::LOG_SUCCESS.with(entry.name.clone()));
          }
          ChannelMessage::Duplicate(conflict, to_install, entry) => {
            Modal::new("Overwrite existing?")
              .with_content(format!(
                "Encountered conflict when trying to install {}",
                entry.id
              ))
              .with_content(match conflict {
                StringOrPath::String(id) => format!("A mod with ID {} alread exists.", id),
                StringOrPath::Path(path) => format!(
                  "A folder already exists at the path {}.",
                  path.to_string_lossy()
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
              .with_button(
                "Overwrite",
                ModList::OVERWRITE.with((
                  match conflict {
                    StringOrPath::String(id) => data.mod_list.mods.get(id).unwrap().path.clone(),
                    StringOrPath::Path(path) => path.clone(),
                  },
                  to_install.clone(),
                  entry.clone(),
                )),
              )
              .show(ctx, env, &());
          }
          ChannelMessage::Error(name, err) => {
            ctx.submit_command(App::LOG_ERROR.with((name.clone(), err.clone())));
            eprintln!("Failed to install {}", err);
          }
        }
      }
    } else if let Event::Notification(notif) = event {
      if let Some(entry) = notif.get(ModEntry::AUTO_UPDATE) {
        Modal::new("Auto-update?")
          .with_content(format!("Would you like to automatically update {}?", entry.name))
          .with_content(format!("Installed version: {}", entry.version))
          .with_content(format!(
            "New version: {}",
            entry
              .remote_version
              .as_ref()
              .map(|v| v.version.to_string())
              .unwrap_or_else(|| String::from(
                "Error: failed to retrieve version, this shouldn't be possible."
              ))
          ))
          .with_content(
            Maybe::or_empty(|| Label::wrapped("\
              NOTE: A .git directory has been detected in the target directory. \
              Are you sure this isn't being used for development?\
            "))
            .lens(
              lens::Constant(data.settings.git_warn.then(|| {
                if entry.path.join(".git").exists() {
                  Some(())
                } else {
                  None
                }
              }).flatten())
            )
            .boxed()
          )
          .with_content("WARNING:")
          .with_content("Save compatibility is not guaranteed when updating a mod. Your save may no longer load if you apply this update.")
          .with_content("Bug reports about saves broken by using this feature will be ignored.")
          .with_content("YOU HAVE BEEN WARNED")
          .with_button("Update", ModList::AUTO_UPDATE.with(entry.clone()))
          .with_close_label("Cancel")
          .show_with_size(ctx, env, &(), (600., 300.));
      }
    }

    child.event(ctx, event, data, env)
  }
}

struct AppController;

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
              SettingsCommand::UpdateInstallDir(handle.clone()),
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
          let widget = if dbg!(self_update()).is_ok() {
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
          if bump_is_greater(local_tag, release_tag).is_ok_with(|b| *b) {
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

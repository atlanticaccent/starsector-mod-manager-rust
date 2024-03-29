use std::{
  fs::{metadata, File},
  io::Write,
  path::PathBuf,
  rc::Rc,
  sync::Arc,
};

use base64::{decode, encode};
use chrono::{DateTime, Local, TimeZone};
use druid::{
  commands,
  im::{OrdMap, Vector},
  keyboard_types::Key,
  lens,
  widget::{
    Axis, Button, Checkbox, Either, Flex, Label, List, Maybe, Scope, SizedBox, Spinner, Tabs,
    TabsPolicy, TextBox, ViewSwitcher,
  },
  AppDelegate as Delegate, Command, Data, DelegateCtx, Env, Event, EventCtx, Handled, KeyEvent,
  Lens, LensExt, Selector, SingleUse, Size, Target, Widget, WidgetExt, WidgetId, WindowDesc,
  WindowHandle, WindowId, WindowLevel,
};
use druid_widget_nursery::{
  material_icons::Icon, FutureWidget, ProgressBar, Separator, Stack, StackChildPosition,
  WidgetExt as WidgetExtNursery,
};
use rand::random;
use remove_dir_all::remove_dir_all;
use reqwest::Url;
use strum::IntoEnumIterator;
use tap::{Pipe, Tap};
use tokio::runtime::Handle;
use webview_shared::{
  InstallType, UserEvent, FRACTAL_INDEX, FRACTAL_MODDING_SUBFORUM, FRACTAL_MODS_FORUM, PROJECT,
  WEBVIEW_EVENT, WEBVIEW_INSTALL, WEBVIEW_OFFSET,
};
use webview_subsystem::init_webview;
use wry::WebView;

use crate::{
  app::util::{option_ptr_cmp, WidgetExtEx},
  patch::{
    split::Split,
    tabs_policy::{InitialTab, StaticTabsForked},
  },
};

use self::{
  controllers::{AppController, HoverController, InstallController, ModListController},
  installer::{HybridPath, StringOrPath, DOWNLOAD_PROGRESS, DOWNLOAD_STARTED, INSTALL_ALL},
  mod_description::ModDescription,
  mod_entry::{ModEntry, ModMetadata},
  mod_list::{EnabledMods, Filters, ModList},
  mod_repo::ModRepo,
  modal::Modal,
  settings::{Settings, SettingsCommand},
  util::{
    button_painter, get_latest_manager, get_quoted_version, get_starsector_version, h2, h3,
    icons::*, make_column_pair, Button2, CommandExt, DummyTransfer, IndyToggleState, LabelExt,
    LensExtExt as _, Release, GET_INSTALLED_STARSECTOR,
  },
};

mod controllers;
pub mod installer;
mod mod_description;
mod mod_entry;
mod mod_list;
mod mod_repo;
pub mod modal;
mod settings;
mod updater;
#[allow(dead_code)]
#[path = "./util.rs"]
pub mod util;

const TAG: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Data, Lens)]
pub struct App {
  init: bool,
  settings: settings::Settings,
  mod_list: mod_list::ModList,
  active: Option<String>,
  #[data(ignore)]
  runtime: Handle,
  #[data(ignore)]
  widget_id: WidgetId,
  #[data(same_fn = "PartialEq::eq")]
  log: Vector<String>,
  overwrite_log: Vector<Rc<(StringOrPath, HybridPath, Arc<ModEntry>)>>,
  duplicate_log: Vector<(Arc<ModEntry>, Arc<ModEntry>)>,
  #[data(same_fn = "option_ptr_cmp")]
  webview: Option<Rc<WebView>>,
  downloads: OrdMap<i64, (i64, String, f64)>,
  mod_repo: Option<ModRepo>,
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
  const CONFIRM_DELETE_MOD: Selector<Arc<ModEntry>> = Selector::new("app.mod_entry.delete");
  const REMOVE_DOWNLOAD_BAR: Selector<i64> = Selector::new("app.download.bar.remove");
  const FOUND_MULTIPLE: Selector<(HybridPath, Vec<PathBuf>)> =
    Selector::new("app.install.found_multiple");

  pub fn new(runtime: Handle) -> Self {
    let settings = settings::Settings::load()
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
      .unwrap_or_else(|_| settings::Settings::new());

    let headings = settings.headings.clone();

    App {
      init: false,
      settings,
      mod_list: mod_list::ModList::new(headings),
      active: None,
      runtime,
      widget_id: WidgetId::reserved(0),
      log: Vector::new(),
      overwrite_log: Vector::new(),
      duplicate_log: Vector::new(),
      webview: None,
      downloads: OrdMap::new(),
      mod_repo: None,
    }
  }

  pub fn ui_builder() -> impl Widget<Self> {
    let settings = Flex::row()
      .with_child(
        Flex::row()
          .with_child(Label::new("Settings").with_text_size(18.))
          .with_spacer(5.)
          .with_child(Icon::new(SETTINGS))
          .padding((8., 4.))
          .background(button_painter())
          .controller(HoverController)
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
          .controller(HoverController)
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
      .controller(HoverController)
      .on_click(|_, _, _| {})
      .controller(InstallController)
      .on_command(App::OPEN_FILE, |ctx, payload, data| {
        if let Some(targets) = payload {
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
              installer::Payload::Initial(targets.iter().map(|f| f.to_path_buf()).collect())
                .install(
                  ctx.get_external_handle(),
                  data.settings.install_dir.clone().unwrap(),
                  data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
                ),
            );
          }
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
      })
      .disabled_if(|data, _| data.settings.install_dir.is_none());
    let browse_index_button = Flex::row()
      .with_child(Label::new("Open Mod Browser").with_text_size(18.))
      .with_spacer(5.)
      .with_child(Icon::new(OPEN_BROWSER))
      .padding((8., 4.))
      .background(button_painter())
      .controller(HoverController)
      .on_click(|event_ctx, _, _| event_ctx.submit_command(App::OPEN_WEBVIEW.with(None)))
      .expand_width()
      .disabled_if(|data: &App, _| data.settings.install_dir.is_none());
    let mod_repo = FutureWidget::new(
      |_, _| ModRepo::get_mod_repo(),
      Flex::row()
        .with_child(Label::new("Open Unofficial Mod Repo").with_text_size(18.))
        .with_spacer(5.)
        .with_child(Icon::new(EXTENSION))
        .padding((8., 4.))
        .background(button_painter()),
      |value, data: &mut App, _| {
        data.mod_repo = value.inspect_err(|err| eprintln!("{:?}", err)).ok();

        Flex::row()
          .with_child(Label::new("Open Unofficial Mod Repo").with_text_size(18.))
          .with_spacer(5.)
          .with_child(Icon::new(EXTENSION))
          .padding((8., 4.))
          .background(button_painter())
          .controller(HoverController)
          .on_click(|ctx, data: &mut App, _| {
            if data.mod_repo.is_some() {
              let modal = Stack::new()
                .with_child(
                  ModRepo::ui_builder().disabled_if(|data: &ModRepo, _| data.modal_open()),
                )
                .with_positioned_child(
                  Either::new(
                    |modal: &Option<String>, _| modal.is_some(),
                    Modal::new("Open in Discord?")
                      .with_content("Attempt to open this link in the Discord app?")
                      .with_button("Open", ModRepo::OPEN_IN_DISCORD)
                      .with_close()
                      .with_on_close_override(|ctx, _| {
                        ctx.submit_command_global(ModRepo::CLEAR_MODAL)
                      })
                      .build()
                      .background(druid::theme::BACKGROUND_DARK)
                      .border(druid::Color::BLACK, 2.)
                      .fix_size(300., 125.),
                    SizedBox::empty(),
                  )
                  .lens(ModRepo::modal),
                  StackChildPosition::new().top(Some(20.)),
                )
                .align(druid::UnitPoint::CENTER)
                .lens(App::mod_repo.map(
                  |data| data.clone().unwrap(),
                  |orig, new| {
                    orig.replace(new);
                  },
                ));

              let window = WindowDesc::new(modal.boxed())
                .window_size((1000., 400.))
                .show_titlebar(false)
                .set_level(WindowLevel::AppWindow);

              ctx.new_window(window);
            }
          })
          .boxed()
      },
    )
    .disabled_if(|data, _| data.mod_repo.is_none());
    let mod_list = ViewSwitcher::new(
      |data: &ModList, _| data.header.headings.clone(),
      |_, _, _| mod_list::ModList::ui_builder().boxed(),
    )
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
      |data: &App, _| {
        (
          data.active.clone(),
          data.mod_list.mods.clone(),
          data.webview.is_some(),
        )
      },
      |(active, mods, enabled), _, _| {
        if let Some(entry) = active.as_ref().and_then(|active| mods.get(active)) {
          let enabled = *enabled;
          ModDescription::ui_builder()
            .lens(lens::Constant(entry.clone()))
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
          .controller(HoverController)
          .on_click(|_, data: &mut App, _| {
            if let Some(install_dir) = data.settings.install_dir.as_ref().cloned() {
              let ids: Vec<String> = data.mod_list.mods.keys().cloned().collect();

              for id in ids.iter() {
                if let Some(mut entry) = data.mod_list.mods.remove(id) {
                  (Arc::make_mut(&mut entry)).enabled = true;
                  data.mod_list.mods.insert(id.clone(), entry);
                }
              }
              if let Err(err) = EnabledMods::from(ids).save(&install_dir) {
                eprintln!("{:?}", err)
              }
            }
          })
          .disabled_if(|data: &App, _| data.mod_list.mods.values().all(|e| e.enabled))
          .expand_width(),
      )
      .with_spacer(5.)
      .with_child(
        Button::new("Disable All")
          .controller(HoverController)
          .on_click(|_, data: &mut App, _| {
            if let Some(install_dir) = data.settings.install_dir.as_ref() {
              let ids: Vec<String> = data.mod_list.mods.keys().cloned().collect();

              for id in ids.iter() {
                if let Some(mut entry) = data.mod_list.mods.remove(id) {
                  (Arc::make_mut(&mut entry)).enabled = false;
                  data.mod_list.mods.insert(id.clone(), entry);
                }
              }
              if let Err(err) = EnabledMods::empty().save(install_dir) {
                eprintln!("{:?}", err)
              }
            }
          })
          .disabled_if(|data: &App, _| data.mod_list.mods.values().all(|e| !e.enabled))
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
              IndyToggleState::default(),
              Checkbox::from_label(Label::wrapped(filter.to_string())).on_change(
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
                .controller(HoverController)
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
      .with_child(Either::new(
        |app: &App, _| app.webview.is_none(),
        Flex::row()
          .with_child(settings)
          .with_spacer(10.)
          .with_child(install_mod_button)
          .with_spacer(10.)
          .with_child(browse_index_button)
          .with_spacer(10.)
          .with_child(mod_repo)
          .with_spacer(10.)
          .with_child(refresh)
          .with_spacer(10.)
          .with_child(
            ViewSwitcher::new(
              |len: &usize, _| *len,
              |len, _, _| Box::new(h3(&format!("Installed: {}", len))),
            )
            .lens(App::mod_list.then(ModList::mods).compute(|data| data.len())),
          )
          .with_spacer(10.)
          .with_child(
            ViewSwitcher::new(
              |len: &usize, _| *len,
              |len, _, _| Box::new(h3(&format!("Active: {}", len))),
            )
            .lens(
              App::mod_list
                .then(ModList::mods)
                .compute(|data| data.values().filter(|e| e.enabled).count()),
            ),
          )
          .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
          .expand_width(),
        Flex::row()
          .with_child(
            Flex::row()
              .with_child(Label::new("Mod Index").with_text_size(18.))
              .with_spacer(5.)
              .with_child(Icon::new(NAVIGATE_NEXT))
              .padding((8., 4.))
              .background(button_painter())
              .controller(HoverController)
              .on_click(|_, data: &mut App, _| {
                if let Some(webview) = &data.webview {
                  if webview.url().as_str() != FRACTAL_INDEX {
                    webview.load_url(FRACTAL_INDEX)
                  }
                }
              }),
          )
          .with_spacer(10.)
          .with_child(
            Flex::row()
              .with_child(Label::new("Mods Subforum").with_text_size(18.))
              .with_spacer(5.)
              .with_child(Icon::new(NAVIGATE_NEXT))
              .padding((8., 4.))
              .background(button_painter())
              .controller(HoverController)
              .on_click(|_, data: &mut App, _| {
                if let Some(webview) = &data.webview {
                  if webview.url().as_str() != FRACTAL_MODS_FORUM {
                    webview.load_url(FRACTAL_MODS_FORUM)
                  }
                }
              })
          )
          .with_spacer(10.)
          .with_child(
            Flex::row()
              .with_child(Label::new("Modding Subforum").with_text_size(18.))
              .with_spacer(5.)
              .with_child(Icon::new(NAVIGATE_NEXT))
              .padding((8., 4.))
              .background(button_painter())
              .controller(HoverController)
              .on_click(|_, data: &mut App, _| {
                if let Some(webview) = &data.webview {
                  if webview.url().as_str() != FRACTAL_MODDING_SUBFORUM {
                    webview.load_url(FRACTAL_MODDING_SUBFORUM)
                  }
                }
              }),
          )
          .with_flex_spacer(1.0)
          .with_child(
            Flex::row()
              .with_child(Label::new("Close Mod Browser").with_text_size(18.))
              .with_spacer(5.)
              .with_child(Icon::new(CLOSE))
              .padding((8., 4.))
              .background(button_painter())
              .controller(HoverController)
              .on_click(|ctx, data: &mut App, _| {
                data
                  .webview
                  .as_mut()
                  .inspect(|webview| webview.set_visible(false));
                data.webview = None;
                ctx.submit_command(App::ENABLE)
              }),
          ),
      ))
      .with_spacer(20.)
      .with_flex_child(
        Split::columns(mod_list, side_panel)
          .split_point(0.8)
          .draggable(true)
          .expand_height()
          .on_event(|ctx, event, _| {
            if let Event::Command(cmd) = event {
              if (cmd.is(ModList::SUBMIT_ENTRY) || cmd.is(App::ENABLE)) && ctx.is_disabled() {
                ctx.set_disabled(false);
              } else if cmd.is(App::DISABLE) {
                ctx.set_disabled(true);
              }
            }
            false
          }),
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
  ) -> anyhow::Result<()> {
    let child = Self::launch(&install_dir, experimental_launch, resolution).await?;

    child.wait_with_output().await?;

    Ok(())
  }

  #[cfg(any(target_os = "windows", target_os = "linux"))]
  async fn launch(
    install_dir: &PathBuf,
    experimental_launch: bool,
    resolution: (u32, u32),
  ) -> anyhow::Result<tokio::process::Child> {
    use tokio::fs::read_to_string;
    use tokio::process::Command;

    Ok(if experimental_launch {
      #[cfg(target_os = "windows")]
      let vmparams_path = install_dir.join("vmparams");
      #[cfg(target_os = "linux")]
      let vmparams_path = install_dir.join("starsector.sh");

      let args_raw = read_to_string(vmparams_path).await?;
      let args: Vec<&str> = args_raw.split_ascii_whitespace().skip(1).collect();

      #[cfg(target_os = "windows")]
      let executable = install_dir.join("jre/bin/java.exe");
      #[cfg(target_os = "linux")]
      let executable = install_dir.join("jre_linux/bin/java");

      #[cfg(target_os = "windows")]
      let current_dir = install_dir.join("starsector-core");
      #[cfg(target_os = "linux")]
      let current_dir = install_dir.clone();

      Command::new(executable)
        .current_dir(current_dir)
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
      #[cfg(target_os = "windows")]
      let executable = install_dir.join("starsector.exe");
      #[cfg(target_os = "linux")]
      let executable = install_dir.join("starsector.sh");

      Command::new(executable)
        .current_dir(install_dir)
        .spawn()
        .expect("Execute Starsector")
    })
  }

  #[cfg(target_os = "macos")]
  async fn launch(
    install_dir: &std::path::Path,
    experimental_launch: bool,
    resolution: (u32, u32),
  ) -> anyhow::Result<tokio::process::Child> {
    use anyhow::Context;
    use tokio::process::Command;

    Ok(if experimental_launch {
      Command::new(install_dir.join("Contents/MacOS/starsector_macos.sh"))
        .current_dir(install_dir.join("Contents/MacOS"))
        .env(
          "EXTRAARGS",
          format!(
            "-DlaunchDirect=true -DstartRes={}x{} -DstartFS=false -DstartSound=true",
            resolution.0, resolution.1
          ),
        )
        .spawn()
        .expect("Execute Starsector")
    } else {
      let executable = install_dir.parent().context("Get install_dir parent")?;
      let current_dir = executable.parent().context("Get install_dir parent")?;

      Command::new(executable)
        .current_dir(current_dir)
        .spawn()
        .expect("Execute Starsector")
    })
  }

  fn log_message(&mut self, message: &str) {
    self
      .log
      .push_back(format!("[{}] {}", Local::now().format("%H:%M:%S"), message))
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
  UpdateModDescription(String),
}

#[derive(Default)]
pub struct AppDelegate {
  settings_id: Option<WindowId>,
  root_id: Option<WindowId>,
  root_window: Option<WindowHandle>,
  log_window: Option<WindowId>,
  overwrite_window: Option<WindowId>,
  duplicate_window: Option<WindowId>,
  download_window: Option<WindowId>,
  mega_file: Option<(File, PathBuf)>,
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

          let settings_window =
            WindowDesc::new(settings::Settings::ui_builder().lens(App::settings))
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
    } else if let Some(url) = cmd.get(App::OPEN_WEBVIEW) && let Some(window) = self.root_window.as_ref() {
      ctx.submit_command(App::DISABLE);
      let webview = init_webview(url.clone(), window, ctx.get_external_handle()).expect("Initialize webview");

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
    } else if let Some(user_event) = cmd.get(WEBVIEW_EVENT) && let Some(webview) = &data.webview {
      match user_event {
        UserEvent::Navigation(uri) => {
          println!("Navigation: {}", uri);
          if uri.starts_with("https://www.mediafire.com/file") {
            let _ = webview.evaluate_script(r#"window.alert("You appear to be on a Mediafire site.\nIn order to correctly trigger a Mediafire download, attempt to open the dowload link in a new window.\nThis can be done through the right click context menu, or using a platform shortcut.")"#);
          }
        },
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
        },
        UserEvent::Download(uri) => {
          let _ = webview.evaluate_script("location.reload();");
          ctx.submit_command(WEBVIEW_INSTALL.with(InstallType::Uri(uri.clone())))
        },
        UserEvent::CancelDownload => {},
        UserEvent::NewWindow(uri) => {
          webview.evaluate_script(&format!("window.location.assign('{}')", uri)).expect("Navigate webview");
        },
        UserEvent::BlobReceived(uri) => {
          let path = PROJECT.cache_dir().join(format!("{}", random::<u16>()));
          self.mega_file = Some((File::create(&path).expect("Create file"), path));
          webview.evaluate_script(&format!(r#"
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
          "#, uri)).expect("Eval script");
        },
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
              },
              None => {
                ctx
                .submit_command(
                  WEBVIEW_INSTALL.with(
                  InstallType::Path(path.clone()))
                );
                self.mega_file = None;
              }
            }
          }
        },
      }
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
        return None;
      }
      Event::WindowSize(Size { width, height }) => {
        if Some(window_id) == self.root_id && let Some(webview) = &data.webview {
          webview.set_bounds(wry::Rect {
            x: 0,
            y: WEBVIEW_OFFSET.into(),
            width: width as u32,
            height: height as u32,
          })
        }
      }
      _ => {}
    }

    Some(event)
  }
}

impl AppDelegate {
  fn build_log_window() -> impl Widget<App> {
    let modal = Modal::new("Log").with_content("").with_content(
      List::new(|| Label::wrapped_func(|val: &String, _| val.clone()))
        .lens(App::log)
        .boxed(),
    );

    modal.with_button("Close", App::CLEAR_LOG).build().boxed()
  }

  fn display_if_closed(&mut self, ctx: &mut DelegateCtx, window_type: SubwindowType) {
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

  fn build_overwrite_window() -> impl Widget<App> {
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

  fn make_dupe_col(dupe_a: &Arc<ModEntry>, dupe_b: &Arc<ModEntry>) -> Flex<App> {
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

  fn build_progress_bars() -> impl Widget<App> {
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
                    Icon::new(VERIFIED),
                  )
                  .lens(lens!((i64, String, f64), 2)),
                )
                .with_child(
                  Either::new(
                    |fraction, _| *fraction < 1.0,
                    Icon::new(CLOSE).with_color(druid::Color::GRAY),
                    Icon::new(CLOSE),
                  )
                  .lens(lens!((i64, String, f64), 2))
                  .controller(HoverController)
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

  fn build_found_multiple(source: HybridPath, found_paths: Vec<PathBuf>) -> impl Widget<App> {
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

enum SubwindowType {
  Log,
  Overwrite,
  Duplicate,
  Download,
}

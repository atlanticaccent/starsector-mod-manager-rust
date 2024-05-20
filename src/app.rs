use std::{path::PathBuf, rc::Rc};

use chrono::Local;
use druid::{
  im::{OrdMap, Vector},
  lens,
  widget::{Flex, Maybe, Scope, WidgetWrapper, ZStack},
  Data, Lens, LensExt, Selector, SingleUse, Widget, WidgetExt, WidgetId,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as WidgetExtNursery};
use tokio::runtime::Handle;
use webview_shared::PROJECT;

use self::{
  activity::Activity,
  browser::Browser,
  controllers::{AppController, HoverController, ModListController},
  installer::{HybridPath, StringOrPath},
  mod_description::ModDescription,
  mod_entry::{ModEntry, UpdateStatus, ViewModEntry},
  mod_list::ModList,
  mod_repo::ModRepo,
  overlays::Popup,
  settings::Settings,
  tools::Tools,
  util::{bold_text, icons::*, xxHashMap, Release},
};
use crate::{
  app::util::WidgetExtEx,
  nav_bar::{Nav, NavBar, NavLabel},
  patch::{
    tabs::tab::{InitialTab, Tabs, TabsPolicy},
    tabs_policy::StaticTabsForked,
  },
  theme::{Theme, CHANGE_THEME},
  widgets::root_stack::RootStack,
};

mod activity;
pub mod app_delegate;
mod browser;
pub mod controllers;
pub mod installer;
mod mod_description;
pub mod mod_entry;
pub mod mod_list;
mod mod_repo;
pub mod modal;
pub mod overlays;
mod settings;
mod tools;
mod updater;
#[allow(dead_code)]
#[path = "./util.rs"]
pub mod util;

const TAG: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Data, Lens)]
pub struct App {
  init: bool,
  pub settings: settings::Settings,
  pub mod_list: mod_list::ModList,
  active: Option<String>,
  #[data(ignore)]
  runtime: Handle,
  #[data(ignore)]
  widget_id: WidgetId,
  #[data(same_fn = "PartialEq::eq")]
  log: Vector<String>,
  overwrite_log: Vector<Rc<(StringOrPath, HybridPath, ModEntry)>>,
  duplicate_log: Vector<(ViewModEntry, ViewModEntry)>,
  browser: Browser,
  downloads: OrdMap<i64, (i64, String, f64)>,
  mod_repo: Option<ModRepo>,
  pub popups: Vector<Popup>,
}

impl App {
  const SELECTOR: Selector<app_delegate::AppCommands> = Selector::new("app.update.commands");
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
  const LOG_OVERWRITE: Selector<(StringOrPath, HybridPath, ModEntry)> =
    Selector::new("app.mod.install.overwrite");
  const REMOVE_DUPLICATE_LOG_ENTRY: Selector<String> =
    Selector::new("app.mod.duplicate.remove_log");
  pub const OPEN_WEBVIEW: Selector<Option<String>> = Selector::new("app.webview.open");
  const CONFIRM_DELETE_MOD: Selector<ModEntry> = Selector::new("app.mod_entry.delete");
  const REMOVE_DOWNLOAD_BAR: Selector<i64> = Selector::new("app.download.bar.remove");
  const FOUND_MULTIPLE: Selector<(HybridPath, Vec<PathBuf>)> =
    Selector::new("app.install.found_multiple");

  const TOGGLE_NAV_BAR: Selector = Selector::new("app.nav_bar.collapse");
  const REPLACE_MODS: Selector<SingleUse<xxHashMap<String, ModEntry>>> =
    Selector::new("app.mod_list.replace");

  pub fn new(runtime: Handle) -> Self {
    let settings = settings::Settings::load()
      .map(|mut settings| {
        if let Some(install_dir) = settings.install_dir.clone() {
          settings.install_dir_buf = install_dir.to_string_lossy().to_string();
          settings.vmparams = tools::vmparams::VMParams::load(install_dir).ok();
        }
        settings
      })
      .unwrap_or_else(|e| {
        dbg!(e);
        settings::Settings::new()
      });

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
      browser: Default::default(),
      downloads: OrdMap::new(),
      mod_repo: None,
      popups: Vector::new(),
    }
  }

  pub fn replace_mods(&mut self, mods: xxHashMap<String, ModEntry>) {
    self.mod_list.replace_mods(mods)
  }

  pub fn view() -> impl Widget<Self> {
    let nav_bar = ZStack::new(
      Flex::<bool>::column()
        .with_default_spacer()
        .with_child(
          bold_text(
            "MOSS",
            36.0,
            druid::text::FontWeight::BOLD,
            druid::theme::TEXT_COLOR,
          )
          .align_horizontal(druid::UnitPoint::CENTER)
          .expand_width(),
        )
        .with_spacer(10.0)
        .with_child(NavBar::new(
          Nav::new(NavLabel::Root).as_root().with_children(vec![
            Nav::new(NavLabel::Mods)
              .overridden(false)
              .with_children(Some(Nav::new(NavLabel::ModDetails))),
            Nav::new(NavLabel::Profiles),
            Nav::new(NavLabel::Performance),
            Nav::new(NavLabel::ModBrowsers)
              .with_children(vec![
                Nav::new(NavLabel::Starmodder)
                  .overridden(false)
                  .with_children(Some(Nav::new(NavLabel::StarmodderDetails))),
                Nav::new(NavLabel::WebBrowser),
              ])
              .linked_to(NavLabel::Starmodder)
              .is_always_open(),
            Nav::separator(),
            Nav::new(NavLabel::Settings),
          ]),
          NavLabel::Mods,
        ))
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
        .must_fill_main_axis(true),
    )
    .with_aligned_child(
      Icon::new(*FIRST_PAGE)
        .fix_size(34., 34.)
        .controller(HoverController::default())
        .on_click(|ctx, _, _| ctx.submit_command(App::TOGGLE_NAV_BAR))
        .padding(6.),
      druid::UnitPoint::BOTTOM_RIGHT,
    )
    .expand();

    Flex::row()
      .with_child(
        nav_bar
          .fix_width(175.)
          .else_if(
            |data, _| !data,
            Icon::new(*LAST_PAGE)
              .fix_size(34., 34.)
              .controller(HoverController::default())
              .on_click(|ctx, _, _| ctx.submit_command(App::TOGGLE_NAV_BAR))
              .padding(6.)
              .align_vertical(druid::UnitPoint::BOTTOM)
              .expand_height(),
          )
          .on_command(App::TOGGLE_NAV_BAR, |_, _, data| *data = !*data)
          .scope_independent(|| true),
      )
      .with_flex_child(
        Tabs::for_policy(StaticTabsForked::build(vec![
          InitialTab::new(
            NavLabel::Mods,
            ModList::view()
              .lens(App::mod_list)
              .on_change(ModList::on_app_data_change)
              .controller(ModListController),
          ),
          InitialTab::new(
            NavLabel::ModDetails,
            Maybe::new(
              || ModDescription::view(),
              || ModDescription::empty_builder(),
            )
            .lens(lens::Map::new(
              |app: &App| {
                app
                  .active
                  .as_ref()
                  .and_then(|id| app.mod_list.mods.get(id).cloned())
              },
              |app, entry| {
                if let Some(entry) = entry {
                  app.mod_list.mods.insert(entry.id.clone(), entry);
                }
              },
            )),
          ),
          InitialTab::new(
            NavLabel::Performance,
            Tools::view()
              .lens(Tools::settings_sync())
              .on_change(Settings::save_on_change)
              .lens(App::settings),
          ),
          InitialTab::new(NavLabel::WebBrowser, Browser::view().lens(App::browser)),
          InitialTab::new(NavLabel::Activity, Activity::view().lens(App::log)),
          InitialTab::new(NavLabel::Settings, Settings::view().lens(App::settings)),
        ]))
        .with_transition(crate::patch::tabs::tab::TabsTransition::Instant)
        .scope_with(false, |widget| {
          widget
            .on_command2(Nav::NAV_SELECTOR, |tabs, ctx, label, state| {
              let tabs = tabs.wrapped_mut();
              let rebuild = &mut state.inner;
              if *label != NavLabel::ModDetails {
                ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::Mods, false)));
                ctx.submit_command(NavBar::REMOVE_OVERRIDE.with(NavLabel::ModDetails))
              }
              if *label != NavLabel::StarmodderDetails {
                ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::Starmodder, false)));
                ctx.submit_command(NavBar::REMOVE_OVERRIDE.with(NavLabel::StarmodderDetails))
              }

              match label {
                NavLabel::Mods => {
                  tabs.set_tab_index_by_label(NavLabel::Mods);
                  if *rebuild {
                    ctx.submit_command(ModList::REBUILD);
                    *rebuild = false;
                  }
                }
                NavLabel::ModDetails => {
                  ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::Mods, true)));
                  ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::ModDetails, true)));
                  tabs.set_tab_index_by_label(NavLabel::ModDetails)
                }
                label @ (NavLabel::WebBrowser | NavLabel::Performance | NavLabel::Settings) => {
                  tabs.set_tab_index_by_label(label)
                }
                _ => eprintln!("Failed to open an item for a nav bar control"),
              }
              true
            })
            .on_command(ModList::REBUILD_NEXT_PASS, |_, _, state| {
              state.inner = true;
            })
        })
        .on_command(util::MASTER_VERSION_RECEIVED, |_ctx, (id, res), data| {
          let remote = res.as_ref().ok().cloned();
          let entry_lens = App::mod_list.then(ModList::mods).deref().index(id);

          if let Some(version_checker) =
            entry_lens.clone().then(ModEntry::version_checker).get(data)
          {
            entry_lens
              .clone()
              .then(ModEntry::remote_version)
              .put(data, remote.clone());

            entry_lens
              .then(ModEntry::update_status)
              .put(data, Some(UpdateStatus::from((&version_checker, &remote))))
          }
        }),
        1.0,
      )
  }

  fn overlay() -> impl Widget<App> {
    Popup::overlay(RootStack::new(Self::view())).controller(AppController)
  }

  pub fn theme_wrapper(theme: Theme) -> impl Widget<Self> {
    Scope::from_lens(
      move |data| (data, theme.clone()),
      lens!((Self, Theme), 0),
      Self::overlay()
        .lens(lens!((Self, Theme), 0))
        .background(druid::theme::WINDOW_BACKGROUND_COLOR)
        .env_scope(|env, (_, theme)| theme.clone().apply(env))
        .on_command(CHANGE_THEME, |_, theme: &Theme, data| {
          data.1 = theme.clone()
        }),
    )
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
    use tokio::{fs::read_to_string, process::Command};

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
}

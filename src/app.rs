use std::path::PathBuf;

use druid::{
  im::{HashSet, Vector},
  widget::{Flex, WidgetWrapper, ZStack},
  Data, Lens, LensExt, Selector, SingleUse, Widget, WidgetExt, WidgetId,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as WidgetExtNursery};
use mod_description::ENABLE_DEPENDENCIES;
use mod_entry::GameVersion;
use settings::ThemeEditor;
use tokio::runtime::Handle;
use util::Tap;
use webview_shared::PROJECT;

use self::{
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
  util::{bold_text, icons::*, FastImMap, Release},
};
pub use crate::theme;
use crate::{
  app::util::WidgetExtEx,
  nav_bar::{Nav, NavBar, NavLabel},
  patch::{
    tabs::tab::{InitialTab, Tabs, TabsPolicy},
    tabs_policy::StaticTabsForked,
  },
  theme::Themes,
  widgets::root_stack::RootStack,
  ENV_STATE,
};

pub mod app_delegate;
mod browser;
pub mod controllers;
pub mod installer;
mod launch;
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
  active: Option<ModDescription<String>>,
  #[data(ignore)]
  runtime: Handle,
  #[data(ignore)]
  widget_id: WidgetId,
  browser: Browser,
  mod_repo: Option<ModRepo>,
  pub popups: Vector<Popup>,
  pub current_tab: NavLabel,
  pub block_next_root_stack: bool,
  views: HashSet<NavLabel>,
  current_view: NavLabel,
}

impl App {
  const SELECTOR: Selector<app_delegate::AppCommands> = Selector::new("app.update.commands");
  const OPEN_FILE: Selector<Option<Vec<PathBuf>>> = Selector::new("app.open.multiple");
  const OPEN_FOLDER: Selector<Option<PathBuf>> = Selector::new("app.open.folder");
  pub const ENABLE: Selector<()> = Selector::new("app.enable");
  const DUMB_UNIVERSAL_ESCAPE: Selector<()> = Selector::new("app.universal_escape");
  const REFRESH: Selector<()> = Selector::new("app.mod_list.refresh");
  const DISABLE: Selector<()> = Selector::new("app.disable");
  const UPDATE_AVAILABLE: Selector<anyhow::Result<Release>> = Selector::new("app.update.available");
  const SELF_UPDATE: Selector<()> = Selector::new("app.update.perform");
  const RESTART: Selector<PathBuf> = Selector::new("app.update.restart");
  const LOG_ERROR: Selector<(String, String)> = Selector::new("app.mod.install.fail");
  const LOG_MESSAGE: Selector<String> = Selector::new("app.mod.install.start");
  const LOG_OVERWRITE: Selector<(StringOrPath, HybridPath, ModEntry)> =
    Selector::new("app.mod.install.overwrite");
  pub const OPEN_WEBVIEW: Selector<Option<String>> = Selector::new("app.webview.open");
  const CONFIRM_DELETE_MOD: Selector<ModEntry> = Selector::new("app.mod_entry.delete");
  const REMOVE_DOWNLOAD_BAR: Selector<i64> = Selector::new("app.download.bar.remove");
  const FOUND_MULTIPLE: Selector<(HybridPath, Vec<PathBuf>)> =
    Selector::new("app.install.found_multiple");

  const TOGGLE_NAV_BAR: Selector = Selector::new("app.nav_bar.collapse");
  const REPLACE_MODS: Selector<SingleUse<FastImMap<String, ModEntry>>> =
    Selector::new("app.mod_list.replace");

  pub fn new(runtime: Handle) -> Self {
    let settings = settings::Settings::load()
      .map(|mut settings| {
        if let Some(install_dir) = settings.install_dir.clone() {
          settings.install_dir_buf = install_dir.to_string_lossy().to_string();
          settings.vmparams =
            tools::vmparams::VMParams::load(install_dir, settings.vmparams_linked).ok();
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
      browser: Default::default(),
      mod_repo: None,
      popups: Vector::new(),
      current_tab: NavLabel::Mods,
      block_next_root_stack: false,
      views: HashSet::new().tap(|s| s.insert(NavLabel::Mods)),
      current_view: NavLabel::Mods,
    }
  }

  pub fn replace_mods(&mut self, mods: FastImMap<String, ModEntry>) {
    self.mod_list.replace_mods(mods)
  }

  pub fn view() -> impl Widget<Self> {
    let nav_bar = ZStack::new(
      Flex::column()
        .with_spacer(15.0)
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
        .with_child(launch::launch_button())
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
                Nav::new(NavLabel::Starmodder),
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
      .with_child(nav_bar.fix_width(195.).scope_with(
        |_| true,
        |widget| {
          widget
            .else_if(
              |data, _| !data.inner,
              Icon::new(*LAST_PAGE)
                .fix_size(34., 34.)
                .controller(HoverController::default())
                .on_click(|ctx, _, _| ctx.submit_command(App::TOGGLE_NAV_BAR))
                .padding(6.)
                .align_vertical(druid::UnitPoint::BOTTOM)
                .expand_height(),
            )
            .on_command(App::TOGGLE_NAV_BAR, |_, _, data| data.inner = !data.inner)
        },
      ))
      .with_flex_child(
        Tabs::for_policy(StaticTabsForked::build(vec![
          InitialTab::new(
            NavLabel::Mods,
            ModList::view()
              .lens(App::mod_list)
              .on_change(ModList::on_app_data_change)
              .controller(ModListController),
          ),
          InitialTab::new(NavLabel::ModDetails, ModDescription::wrapped_view()),
          InitialTab::new(
            NavLabel::Performance,
            Tools::view()
              .lens(Tools::settings_sync())
              .on_change(Settings::save_on_change)
              .lens(App::settings),
          ),
          InitialTab::new(NavLabel::Starmodder, ModRepo::wrapper()),
          InitialTab::new(NavLabel::WebBrowser, Browser::view().lens(App::browser)),
          InitialTab::new(NavLabel::Settings, Settings::view().lens(App::settings)),
          InitialTab::new(
            NavLabel::ThemeEditor,
            ThemeEditor::view()
              .lens(Settings::custom_theme)
              .on_change(Settings::save_on_change)
              .lens(App::settings),
          ),
        ]))
        .with_transition(crate::patch::tabs::tab::TabsTransition::Instant)
        .scope_with(
          |_| false,
          |widget| {
            widget
              .on_command2(Nav::NAV_SELECTOR, |tabs, ctx, label, state| {
                let tabs = tabs.wrapped_mut();
                let rebuild = &mut state.inner;
                state.outer.current_tab = label.clone();
                if *label != NavLabel::ModDetails {
                  ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::Mods, false)));
                  ctx.submit_command(NavBar::REMOVE_OVERRIDE.with(NavLabel::ModDetails))
                }
                if *label != NavLabel::StarmodderDetails {
                  ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::Starmodder, false)));
                  ctx.submit_command(NavBar::REMOVE_OVERRIDE.with(NavLabel::StarmodderDetails))
                }
                if *label != NavLabel::ThemeEditor {
                  ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::ThemeEditor, false)));
                  ctx.submit_command(NavBar::REMOVE_OVERRIDE.with(NavLabel::Settings))
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
                  NavLabel::WebBrowser => {
                    ctx.submit_command(NavBar::RECURSE_SET_EXPANDED.with(NavLabel::WebBrowser));
                    tabs.set_tab_index_by_label(NavLabel::WebBrowser)
                  }
                  NavLabel::ThemeEditor => {
                    ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::Settings, true)));
                    ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::ThemeEditor, true)));
                    tabs.set_tab_index_by_label(NavLabel::ThemeEditor)
                  }
                  label @ (NavLabel::Performance | NavLabel::Starmodder | NavLabel::Settings) => {
                    tabs.set_tab_index_by_label(label)
                  }
                  _ => eprintln!("Failed to open an item for a nav bar control"),
                }
                true
              })
              .on_command(ModList::REBUILD_NEXT_PASS, |_, _, state| {
                state.inner = true;
              })
          },
        )
        .on_command(util::MASTER_VERSION_RECEIVED, |_ctx, (id, res), data| {
          let remote = res.as_ref().ok().cloned();
          let entry_lens = App::mod_list.then(ModList::mods).deref().index(id);

          if let Some(version_checker) = entry_lens
            .clone()
            .then(ModEntry::version_checker.in_arc())
            .get(data)
          {
            entry_lens
              .clone()
              .then(ModEntry::remote_version.in_arc())
              .put(data, remote.clone());

            entry_lens
              .then(ModEntry::update_status.in_arc())
              .put(data, Some(UpdateStatus::from((&version_checker, &remote))))
          }
        })
        .on_notification(ENABLE_DEPENDENCIES, ModDescription::enable_dependencies),
        1.0,
      )
      .on_command(App::DISABLE, |ctx, _, _| ctx.set_disabled(true))
      .on_command(App::ENABLE, |ctx, _, _| ctx.set_disabled(false))
  }
}

#[extend::ext(name = AppViewExt)]
pub impl<W: Widget<App> + 'static> W {
  fn overlay(self) -> impl Widget<App> {
    RootStack::new(Popup::overlay(self)).controller(AppController)
  }

  fn theme_wrapper(self) -> impl Widget<App> {
    self
      .background(druid::theme::WINDOW_BACKGROUND_COLOR)
      .env_scope(|env, app| {
        let theme = app.settings.theme;
        if theme == Themes::Custom {
          app.settings.custom_theme.clone()
        } else {
          theme.into()
        }
        .apply(env)
      })
  }

  fn env_as_shared_data(self) -> impl Widget<App> {
    self.env_scope(|env, data| env.set(crate::ENV_STATE, data))
  }
}

#[derive(Debug, Clone, Hash)]
pub struct EnvSharedData {
  pub show_discrepancy: bool,
  pub game_version: Option<GameVersion>,
}

impl<'a> From<&'a App> for std::sync::Arc<EnvSharedData> {
  fn from(
    App {
      init: _,
      settings,
      mod_list,
      active: _,
      runtime: _,
      widget_id: _,
      browser: _,
      mod_repo: _,
      popups: _,
      current_tab: _,
      block_next_root_stack: _,
      views: _,
      current_view: _,
    }: &'a App,
  ) -> Self {
    std::sync::Arc::new(EnvSharedData {
      show_discrepancy: settings.show_discrepancies,
      game_version: mod_list.starsector_version.clone(),
    })
  }
}

#[extend::ext(name = SharedFromEnv)]
pub impl druid::Env {
  fn shared_data(&self) -> std::sync::Arc<EnvSharedData> {
    self.get(ENV_STATE)
  }
}

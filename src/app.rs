use std::{path::PathBuf, rc::Rc};

use chrono::Local;
use druid::{
  im::{OrdMap, Vector},
  lens,
  widget::{
    Align, Axis, Button, Checkbox, Either, Flex, Label, Maybe, Scope, SizedBox, TextBox,
    ViewSwitcher, ZStack,
  },
  Data, Event, Lens, LensExt, Selector, SingleUse, Target, Widget, WidgetExt, WidgetId, WindowDesc,
  WindowLevel,
};
use druid_widget_nursery::{
  material_icons::Icon, FutureWidget, Mask, Stack, StackChildPosition,
  WidgetExt as WidgetExtNursery,
};
use strum::IntoEnumIterator;
use tap::Tap;
use tokio::runtime::Handle;
use webview_shared::{FRACTAL_INDEX, FRACTAL_MODDING_SUBFORUM, FRACTAL_MODS_FORUM, PROJECT};
use wry::WebView;

use self::{
  controllers::{AppController, HoverController, InstallController, ModListController},
  installer::{HybridPath, StringOrPath},
  mod_description::ModDescription,
  mod_entry::{ModEntry, UpdateStatus, ViewModEntry},
  mod_list::{EnabledMods, Filters, ModList},
  mod_repo::ModRepo,
  modal::Modal,
  overlays::Popup,
  settings::Settings,
  tools::Tools,
  util::{
    bold_text, button_painter, get_quoted_version, h2_fixed, h3_fixed, icons::*, make_column_pair,
    xxHashMap, CommandExt, IndyToggleState, LabelExt, LensExtExt as _, Release, RootStack,
  },
};
use crate::{
  app::util::WidgetExtEx,
  nav_bar::{Nav, NavBar, NavLabel},
  patch::{
    split::Split,
    tabs::tab::{InitialTab, Tabs, TabsPolicy},
    tabs_policy::StaticTabsForked,
  },
  theme::{Theme, CHANGE_THEME},
};

pub mod app_delegate;
pub mod controllers;
pub mod installer;
mod mod_description;
pub mod mod_entry;
pub mod mod_list;
mod mod_repo;
pub mod modal;
mod overlays;
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
  #[data(ignore)]
  webview: Option<Rc<WebView>>,
  downloads: OrdMap<i64, (i64, String, f64)>,
  mod_repo: Option<ModRepo>,
  popup: Option<Popup>,
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
  const CLEAR_OVERWRITE_LOG: Selector<bool> = Selector::new("app.install.clear_overwrite_log");
  const REMOVE_OVERWRITE_LOG_ENTRY: Selector<StringOrPath> =
    Selector::new("app.install.overwrite.decline");
  const DELETE_AND_SUMBIT: Selector<(PathBuf, ModEntry)> =
    Selector::new("app.mod.duplicate.resolve");
  const REMOVE_DUPLICATE_LOG_ENTRY: Selector<String> =
    Selector::new("app.mod.duplicate.remove_log");
  const CLEAR_DUPLICATE_LOG: Selector = Selector::new("app.mod.duplicate.ignore_all");
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
      popup: None,
    }
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
            Nav::new(NavLabel::Activity),
            Nav::new(NavLabel::Downloads),
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
        Scope::from_lens(
          |_| true,
          lens::Unit,
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
            .on_command(App::TOGGLE_NAV_BAR, |_, _, data| *data = !*data),
        )
        .lens(lens::Unit),
      )
      .with_flex_child(
        Tabs::for_policy(StaticTabsForked::build(vec![
          InitialTab::new(
            "mod_list",
            ModList::view()
              .lens(App::mod_list)
              .on_change(ModList::on_app_data_change)
              .controller(ModListController),
          ),
          InitialTab::new(
            "mod_detail",
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
            "tools",
            Tools::view()
              .lens(Tools::settings_sync())
              .on_change(Settings::save_on_change)
              .lens(App::settings),
          ),
          InitialTab::new("settings", Settings::view().lens(App::settings)),
        ]))
        .on_command2(Nav::NAV_SELECTOR, |tabs, ctx, label, _| {
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
              tabs.set_tab_index(0);
              ctx.submit_command(ModList::REBUILD)
            }
            NavLabel::ModDetails => {
              ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::Mods, true)));
              ctx.submit_command(NavBar::SET_OVERRIDE.with((NavLabel::ModDetails, true)));
              tabs.set_tab_index(1)
            }
            NavLabel::Performance => tabs.set_tab_index(2),
            NavLabel::Settings => tabs.set_tab_index(3),
            _ => eprintln!("Failed to open an item for a nav bar control"),
          }
          true
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
      .controller(AppController)
  }

  fn overlay() -> impl Widget<App> {
    Mask::new(RootStack::new(Self::view()))
      .with_mask(Align::centered(Popup::view().lens(App::popup)))
      .dynamic(|data, _| data.popup.is_some())
      .on_command(Popup::OPEN_POPUP, |_, popup, data| {
        data.popup = Some(popup.clone())
      })
      .on_command(Popup::DISMISS, |_, _, data| data.popup = None)
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

  pub fn view_() -> impl Widget<Self> {
    let refresh = Flex::row()
      .with_child(
        Flex::row()
          .with_child(Label::new("Refresh").with_text_size(18.))
          .with_spacer(5.)
          .with_child(Icon::new(*SYNC))
          .padding((8., 4.))
          .background(button_painter())
          .controller(HoverController::default())
          .on_click(|event_ctx, _, _| event_ctx.submit_command(App::REFRESH)),
      )
      .expand_width();
    let install_dir_browser =
      Settings::install_dir_browser_builder(Axis::Vertical).lens(App::settings);
    let install_mod_button = Flex::row()
      .with_child(Label::new("Install Mod(s)").with_text_size(18.))
      .with_spacer(5.)
      .with_child(Icon::new(*INSTALL_DESKTOP))
      .padding((8., 4.))
      .background(button_painter())
      .controller(HoverController::default())
      .on_click(|_, _, _| {})
      .controller(InstallController)
      .disabled_if(|data, _| data.settings.install_dir.is_none());
    let browse_index_button = Flex::row()
      .with_child(Label::new("Open Mod Browser").with_text_size(18.))
      .with_spacer(5.)
      .with_child(Icon::new(*OPEN_BROWSER))
      .padding((8., 4.))
      .background(button_painter())
      .controller(HoverController::default())
      .on_click(|event_ctx, _, _| event_ctx.submit_command(App::OPEN_WEBVIEW.with(None)))
      .expand_width()
      .disabled_if(|data: &App, _| data.settings.install_dir.is_none());
    let mod_repo = FutureWidget::new(
      |_, _| ModRepo::get_mod_repo(),
      Flex::row()
        .with_child(Label::new("Open Unofficial Mod Repo").with_text_size(18.))
        .with_spacer(5.)
        .with_child(Icon::new(*EXTENSION))
        .padding((8., 4.))
        .background(button_painter()),
      |value, data: &mut App, _| {
        data.mod_repo = value.inspect_err(|err| eprintln!("{:?}", err)).ok();

        Flex::row()
          .with_child(Label::new("Open Unofficial Mod Repo").with_text_size(18.))
          .with_spacer(5.)
          .with_child(Icon::new(*EXTENSION))
          .padding((8., 4.))
          .background(button_painter())
          .controller(HoverController::default())
          .on_click(|ctx, data: &mut App, _| {
            if data.mod_repo.is_some() {
              let modal = Stack::new()
                .with_child(ModRepo::view().disabled_if(|data: &ModRepo, _| data.modal_open()))
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
      |_, _, _| mod_list::ModList::view().boxed(),
    )
    .lens(App::mod_list)
    .on_change(|_ctx, _old, data, _env| {
      if let Some(install_dir) = &data.settings.install_dir {
        let enabled: Vec<ViewModEntry> = data
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
    let tool_panel = Flex::column()
      .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
      .with_child(h2_fixed("Search"))
      .with_child(
        TextBox::new()
          .on_change(|ctx, _, _, _| {
            ctx.submit_command(ModList::SEARCH_UPDATE.with(false));
          })
          .lens(App::mod_list.then(ModList::search_text))
          .expand_width(),
      )
      .with_default_spacer()
      .with_child(h2_fixed("Toggles"))
      .with_child(
        Button::new("Enable All")
          .controller(HoverController::default())
          .on_click(|_, data: &mut App, _| {
            if let Some(install_dir) = data.settings.install_dir.as_ref().cloned() {
              let ids: Vec<String> = data.mod_list.mods.keys().cloned().collect();

              for id in ids.iter() {
                if let Some(mut entry) = data.mod_list.mods.remove(id) {
                  entry.enabled = true;
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
          .controller(HoverController::default())
          .on_click(|_, data: &mut App, _| {
            if let Some(install_dir) = data.settings.install_dir.as_ref() {
              let ids: Vec<String> = data.mod_list.mods.keys().cloned().collect();

              for id in ids.iter() {
                if let Some(mut entry) = data.mod_list.mods.remove(id) {
                  entry.enabled = false;
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
      .with_child(h2_fixed("Filters"))
      .tap_mut(|panel| {
        for filter in Filters::iter() {
          match filter {
            Filters::Enabled => panel.add_child(h3_fixed("Status")),
            Filters::Unimplemented => panel.add_child(h3_fixed("Version Checker")),
            Filters::AutoUpdateAvailable => panel.add_child(h3_fixed("Auto Update Support")),
            _ => {}
          };
          panel.add_child(
            Scope::from_function(
              |state: bool| state,
              IndyToggleState::default(),
              Checkbox::from_label(Label::wrapped(filter.to_string())).on_change(
                move |ctx, _, new, _| {
                  // ctx.submit_command(ModList::FILTER_UPDATE.with((filter, !*new)))
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
        h2_fixed("Starsector Version:"),
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
                .with_flex_child(h2_fixed("Launch Starsector").expand_width(), 2.)
                .with_flex_child(Icon::new(*PLAY_ARROW).expand_width(), 1.)
                .padding((8., 4.))
                .background(button_painter())
                .controller(HoverController::default())
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
              |len, _, _| Box::new(h3_fixed(&format!("Installed: {}", len))),
            )
            .lens(App::mod_list.then(ModList::mods).compute(|data| data.len())),
          )
          .with_spacer(10.)
          .with_child(
            ViewSwitcher::new(
              |len: &usize, _| *len,
              |len, _, _| Box::new(h3_fixed(&format!("Active: {}", len))),
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
              .with_child(Icon::new(*NAVIGATE_NEXT))
              .padding((8., 4.))
              .background(button_painter())
              .controller(HoverController::default())
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
              .with_child(Icon::new(*NAVIGATE_NEXT))
              .padding((8., 4.))
              .background(button_painter())
              .controller(HoverController::default())
              .on_click(|_, data: &mut App, _| {
                if let Some(webview) = &data.webview {
                  if webview.url().as_str() != FRACTAL_MODS_FORUM {
                    webview.load_url(FRACTAL_MODS_FORUM)
                  }
                }
              }),
          )
          .with_spacer(10.)
          .with_child(
            Flex::row()
              .with_child(Label::new("Modding Subforum").with_text_size(18.))
              .with_spacer(5.)
              .with_child(Icon::new(*NAVIGATE_NEXT))
              .padding((8., 4.))
              .background(button_painter())
              .controller(HoverController::default())
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
              .with_child(Icon::new(*CLOSE))
              .padding((8., 4.))
              .background(button_painter())
              .controller(HoverController::default())
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
          .on_event(|_, ctx, event, _| {
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

  fn push_overwrite(&mut self, message: (StringOrPath, HybridPath, ModEntry)) {
    if !self.overwrite_log.iter().any(|val| val.0 == message.0) {
      self.overwrite_log.push_back(Rc::new(message))
    }
  }

  fn push_duplicate(&mut self, duplicates: &(ViewModEntry, ViewModEntry)) {
    self.duplicate_log.push_back(duplicates.clone())
  }
}

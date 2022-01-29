use std::sync::Arc;

use druid::{
  widget::{Button, Flex, FlexParams, CrossAxisAlignment, ViewSwitcher},
  AppDelegate as Delegate, Command, Data, DelegateCtx, Env, Handled, Lens, Selector,
  Target, Widget, WidgetExt, WindowDesc, lens, LensExt, WindowId, commands, Menu, platform_menus,
};
use druid_widget_nursery::WidgetExt as WidgetExtNursery;

use self::{mod_entry::ModEntry, mod_description::ModDescription, settings::{SettingsCommand, Settings}, mod_list::EnabledMods};

mod mod_description;
mod mod_entry;
mod mod_list;
mod settings;
#[path = "./util.rs"]
mod util;

#[derive(Clone, Data, Lens)]
pub struct App {
  init: bool,
  settings: settings::Settings,
  mod_list: mod_list::ModList,
  active: Option<Arc<ModEntry>>
}

impl App {
  const SELECTOR: Selector<AppCommands> = Selector::new("app.update.commands");
  
  pub fn new() -> Self {
    App {
      init: false,
      settings: settings::Settings::load().and_then(|mut settings| {
        if settings.vmparams_enabled {
          if let Some(path) = settings.install_dir.clone() {
            settings.vmparams = settings::vmparams::VMParams::load(path).ok();
          }
        }
        if let Some(install_dir) = settings.install_dir.clone() {
          settings.install_dir_buf = install_dir.to_string_lossy().to_string()
        }
        Ok(settings)
      }).unwrap_or_else(|_| settings::Settings::default()),
      mod_list: mod_list::ModList::new(),
      active: None
    }
  }
  
  pub fn ui_builder() -> impl Widget<Self> {
    Flex::column()
      .with_child(
        Flex::row()
          .with_flex_child(
            Flex::row().with_child(Button::new("Settings").on_click(move |event_ctx, _, _| {
              event_ctx.submit_command(App::SELECTOR.with(AppCommands::OpenSettings))
            })).expand_width(),
            FlexParams::new(1., CrossAxisAlignment::Start)
          )
          .with_flex_child(
            Settings::install_dir_browser_builder().lens(App::settings).expand_width().padding(2.),
            FlexParams::new(2., CrossAxisAlignment::Center)
          )
          .expand_width()
      )
      .with_flex_child(
        mod_list::ModList::ui_builder()
        .lens(App::mod_list)
        .on_change(|_ctx, _old, data, _env| {
          if let Some(install_dir) = &data.settings.install_dir {
            let enabled: Vec<Arc<ModEntry>> = data.mod_list.mods.iter().filter_map(|(_, v)| v.enabled.then(|| v.clone())).collect();
  
            if let Err(err) = EnabledMods::from(enabled).save(install_dir) {
              eprintln!("{:?}", err)
            };
          }
        })
        .expand(),
        2.0,
      )
      .with_flex_child(ViewSwitcher::new(
        |active: &Option<Arc<ModEntry>>, _| active.clone(),
        |active, _, _| {
          if let Some(active) = active {
            Box::new(ModDescription::ui_builder().lens(lens::Constant(active.clone())))
          } else {
            Box::new(ModDescription::empty_builder().lens(lens::Unit))
          }
        }
      ).lens(App::active), 1.0)
      .must_fill_main_axis(true)
  }
}

enum AppCommands {
  OpenSettings,
  UpdateModDescription(Arc<ModEntry>)
}

#[derive(Default)]
pub struct AppDelegate {
  settings_id: Option<WindowId>,
  root_id: Option<WindowId>
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
          let install_dir = lens!(App, settings).then(lens!(settings::Settings, install_dir)).get(data);
          lens!(App, settings)
            .then(lens!(settings::Settings, install_dir_buf))
            .put(data, install_dir.map_or_else(|| "".to_string(), |p| p.to_string_lossy().to_string()));

          let settings_window = WindowDesc::new(settings::Settings::ui_builder().lens(App::settings).on_change(|_, _old, data, _| {
              if let Err(err) = data.settings.clone().save() {
                eprintln!("{:?}", err)
              }
            }))
            .window_size((800., 400.))
            .menu(|_, _, _| Menu::empty()
                .entry(platform_menus::common::copy())
                .entry(platform_menus::common::paste())
            )
            .show_titlebar(false);

          self.settings_id = Some(settings_window.id);

          ctx.new_window(settings_window);
          return Handled::Yes
        },
        AppCommands::UpdateModDescription(desc) => {
          data.active = Some(desc.clone());
          
          return Handled::Yes
        }
      }
    } else if cmd.is(settings::Settings::SELECTOR) {
      match cmd.get_unchecked(settings::Settings::SELECTOR) {
        settings::SettingsCommand::UpdateInstallDir(new_install_dir) => {
          lens!(App, settings).then(lens!(settings::Settings, install_dir)).put(data, Some(new_install_dir.to_path_buf()));
          data.mod_list.parse_mod_folder(&Some(new_install_dir.to_path_buf()));
          return Handled::Yes
        },
      }
    }

    Handled::No
  }

  fn window_removed(&mut self, id: WindowId, _data: &mut App, _env: &Env, ctx: &mut DelegateCtx) {
    if Some(id) == self.settings_id {
      self.settings_id = None;
    } else if Some(id) == self.root_id {
      ctx.submit_command(commands::QUIT_APP)
    }
  }

  fn event(&mut self, ctx: &mut DelegateCtx, window_id: WindowId, event: druid::Event, data: &mut App, _: &Env) -> Option<druid::Event> {
    if let druid::Event::WindowConnected = event {
      if self.root_id.is_none() {
        self.root_id = Some(window_id);
        if data.settings.dirty {
          data.settings.dirty = false;
          ctx.submit_command(Settings::SELECTOR.with(SettingsCommand::UpdateInstallDir(data.settings.install_dir.clone().unwrap_or_default())));
        }
      }
    }

    Some(event)
  }
}

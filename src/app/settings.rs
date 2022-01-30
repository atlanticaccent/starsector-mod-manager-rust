use std::{path::PathBuf, rc::Rc};

use druid::{Widget, Lens, Data, widget::{Flex, Label, TextBox, WidgetExt, ValidationDelegate, TextBoxEvent, Controller, Button, Checkbox, SizedBox, ViewSwitcher}, text::{Formatter, Validation, ValidationError, ParseFormatter}, lens, Selector, EventCtx, Event, Point, theme, LensExt, Menu, MenuItem, FileDialogOptions};
use druid_widget_nursery::{WidgetExt as WidgetExtNursery, DynLens, Wedge};
use serde::{Serialize, Deserialize};
use if_chain::if_chain;

use self::vmparams::{VMParams, Value, Unit};

use super::util::{LoadError, SaveError, make_description_row, LabelExt};

pub mod vmparams;

const TRAILING_PADDING: (f64, f64, f64, f64) = (0., 0., 0., 5.);

#[derive(Clone, Data, Lens, Serialize, Deserialize, Default)]
pub struct Settings {
  #[serde(skip)]
  pub dirty: bool,
  #[data(same_fn="PartialEq::eq")]
  pub install_dir: Option<PathBuf>,
  #[serde(skip)]
  pub install_dir_buf: String,
  #[data(same_fn="PartialEq::eq")]
  pub last_browsed: Option<PathBuf>,
  pub git_warn: bool,
  pub vmparams_enabled: bool,
  #[serde(skip)]
  pub vmparams: Option<vmparams::VMParams>,
  pub experimental_launch: bool,
  pub experimental_resolution: (u32, u32)
}

impl Settings {
  pub const SELECTOR: Selector<SettingsCommand> = Selector::new("SETTINGS");

  pub fn ui_builder() -> impl Widget<Self> {
    Flex::column()
      .with_child(Label::new("Settings").with_text_size(theme::TEXT_SIZE_LARGE).with_font(theme::UI_FONT_BOLD).center().padding(2.).expand_width().background(theme::BACKGROUND_LIGHT).controller(DragWindowController::default()))
      .with_flex_child(
        Flex::column()
          .with_child(Self::install_dir_browser_builder().padding(TRAILING_PADDING))
          .with_child(make_description_row(
            Label::wrapped("Warn when overwriting '.git' folders:"),
            Checkbox::new("").lens(Settings::git_warn)
          ).padding(TRAILING_PADDING))
          .with_child(make_description_row(
            Label::wrapped("Enable vmparams editing:"),
            Checkbox::new("").lens(Settings::vmparams_enabled)
          ).on_change(|_, _old, data, _| {
            if data.vmparams_enabled && data.vmparams.is_none() {
              data.vmparams = data.install_dir.clone().ok_or(LoadError::NoSuchFile).and_then(|p| vmparams::VMParams::load(p)).ok()
            }
          }).padding(TRAILING_PADDING))
          .with_child(ViewSwitcher::new(
            |data: &Settings, _| data.vmparams_enabled,
            |enabled, data, _| {
              if *enabled && data.vmparams.is_some() {
                let vmparam_lens = lens::Identity
                  .then(Settings::vmparams)
                  .map(
                    |u| u.clone().expect("This has to work..."),
                    |u, data| *u = Some(data)
                  );

                return Box::new(
                  Flex::column()
                    .with_child(
                      Flex::row()
                        .with_flex_child(SizedBox::empty().expand_width(), 2.25)
                        .with_flex_child(Label::new("Minimum RAM:").expand_width(), 1.)
                        .with_flex_child(
                          TextBox::new().with_formatter(ParseFormatter::new())
                            .lens(VMParams::heap_init.then(Value::amount))
                            .expand_width(),
                          3.
                        )
                        .with_flex_child(
                          Label::dynamic(|u: &Unit, _| u.to_string())
                            .lens(VMParams::heap_init.then(Value::unit))
                            .controller(UnitController::new(VMParams::heap_init.then(Value::unit)))
                            .expand_width(),
                          0.5
                        )
                    )
                    .with_child(
                      Flex::row()
                        .with_flex_child(SizedBox::empty().expand_width(), 2.25)
                        .with_flex_child(Label::new("Maximum RAM:").expand_width(), 1.)
                        .with_flex_child(
                          TextBox::new().with_formatter(ParseFormatter::new())
                            .lens(VMParams::heap_max.then(Value::amount))
                            .expand_width(),
                          3.
                        )
                        .with_flex_child(
                          Label::dynamic(|u: &Unit, _| u.to_string())
                            .lens(VMParams::heap_max.then(Value::unit))
                            .controller(UnitController::new(VMParams::heap_max.then(Value::unit)))
                            .expand_width(),
                          0.5
                        )
                    )
                    .lens(vmparam_lens)
                    .on_change(|_, _, data, _| {
                      if_chain! {
                        if let Some(install_dir) = data.install_dir.clone();
                        if let Some(vmparams) = data.vmparams.clone();
                        if let Err(err) = vmparams.save(install_dir);
                        then {
                          eprintln!("{:?}", err)
                        }
                      }
                    })
                )
              }
              Box::new(SizedBox::empty())
            }
          ).padding(TRAILING_PADDING))
          .with_child(make_description_row(
            Label::wrapped("Enable experimental direct launch:"),
            Checkbox::new("").lens(Settings::experimental_launch)
          ).padding(TRAILING_PADDING))
          .with_child(ViewSwitcher::new(
            |data: &Settings, _| data.experimental_launch,
            |enabled, _, _| {
              if *enabled {
                let res_lens = lens::Identity
                  .then(Settings::experimental_resolution);

                return Box::new(make_description_row(
                  SizedBox::empty(),
                  Flex::column()
                    .with_child(
                      Flex::row()
                        .with_flex_child(Label::new("Horizontal Resolution:"), 1.)
                        .with_flex_child(
                          TextBox::new().with_formatter(ParseFormatter::new())
                            .lens(res_lens.clone().then(lens!((u32, u32), 0))),
                          1.
                        )
                    )
                    .with_child(
                      Flex::row()
                        .with_flex_child(Label::new("Vertical Resolution:"), 1.)
                        .with_flex_child(
                          TextBox::new().with_formatter(ParseFormatter::new())
                            .lens(res_lens.then(lens!((u32, u32), 1))),
                          1.
                        )
                    )
                ))
              }
              Box::new(SizedBox::empty())
            }
          ).padding(TRAILING_PADDING))
          .padding((10., 10.))
          .expand(),
        1.
      )
      .with_child(Flex::row()
        .with_child(Button::new("Close").on_click(|ctx, _, _| {
          ctx.submit_command(druid::commands::CLOSE_WINDOW)
        }))
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::End)
        .main_axis_alignment(druid::widget::MainAxisAlignment::End)
        .expand_width()
      )
      .expand_height()
  }

  pub fn install_dir_browser_builder() -> impl Widget<Self> {
    let input = TextBox::new()
      .with_formatter(InstallDirFormatter {})
      .delegate(InstallDirDelegate {})
      .lens(lens!(Settings, install_dir_buf));

    make_description_row(
      Label::wrapped("Starsector Install Directory:"),
      Flex::row()
        .with_flex_child(input.expand_width(), 1.)
        .with_child(
          Button::new("Browse...").on_click(|ctx, _, _| {
            ctx.submit_command(Selector::new("druid.builtin.textbox-cancel-editing"));
            ctx.submit_command(druid::commands::SHOW_OPEN_PANEL.with(FileDialogOptions::new()
              .packages_as_directories()
              .select_directories()
            ))
          })
        ).on_command(druid::commands::OPEN_FILE, |ctx, payload, data: &mut Settings| {
          if payload.path().is_dir() {
            // assert!(payload.path().join("mods").exists());
            data.install_dir_buf = payload.path().to_string_lossy().to_string();

            ctx.request_paint();
            ctx.submit_command(Settings::SELECTOR.with(SettingsCommand::UpdateInstallDir(payload.path().to_path_buf())));
          }
        }).on_change(|ctx, _old, data, _| {
          data.save();
        })
    )
  }

  pub  fn path(try_make: bool) -> PathBuf {
    use directories::ProjectDirs;
    use std::fs;

    if let Some(proj_dirs) = ProjectDirs::from("org", "laird", "Starsector Mod Manager") {
      if proj_dirs.config_dir().exists() || (try_make && fs::create_dir_all(proj_dirs.config_dir()).is_ok()) {
        return proj_dirs.config_dir().to_path_buf().join("config.json");
      }
    };
    PathBuf::from(r"./config.json")
  }

  pub  fn load() -> Result<Settings, LoadError> {
    use std::{fs, io::Read};

    let mut config_file = fs::File::open(Settings::path(false))
      .map_err(|_| LoadError::NoSuchFile)?;

    let mut config_string = String::new();
    config_file.read_to_string(&mut config_string)
      .map_err(|_| LoadError::ReadError)?;

    serde_json::from_str::<Settings>(&config_string).map_err(|_| LoadError::FormatError).and_then(|mut settings| {
      settings.dirty = true;
      Ok(settings)
    })
  }

   pub fn save(&self) -> Result<(), SaveError> {
    use std::{fs, io::Write};

    let json = serde_json::to_string_pretty(&self)
      .map_err(|_| SaveError::FormatError)?;

    let mut file = fs::File::create(Settings::path(true))
      .map_err(|_| SaveError::FileError)?;

    file.write_all(json.as_bytes())
      .map_err(|_| SaveError::WriteError)
  }
}

pub enum SettingsCommand {
  UpdateInstallDir(PathBuf),
}

struct InstallDirFormatter {}

impl InstallDirFormatter {
  fn validation_err() -> std::io::Error { std::io::Error::new(std::io::ErrorKind::NotFound, "Not a path") }
}

impl Formatter<String> for InstallDirFormatter {
  fn format(&self, value: &String) -> String {
    value.to_string()
  }
  
  fn validate_partial_input(&self, input: &str, sel: &druid::text::Selection) -> Validation {
    if PathBuf::from(input).exists() {
      Validation::success()
    } else {
      Validation::failure(InstallDirFormatter::validation_err())
        .change_text(input.to_string())
        .change_selection(*sel)
    }
  }
  
  fn value(&self, input: &str) -> Result<String, druid::text::ValidationError> {
    match PathBuf::from(input) {
      path if path.exists() => Ok(input.to_string()),
      _ => Err(ValidationError::new(InstallDirFormatter::validation_err()))
    }
  }
}

struct InstallDirDelegate {}

impl ValidationDelegate for InstallDirDelegate {
  fn event(&mut self, ctx: &mut druid::EventCtx, event: TextBoxEvent, current_text: &str) {
    if let TextBoxEvent::Complete | TextBoxEvent::Changed = event {
      ctx.submit_command(Settings::SELECTOR.with(SettingsCommand::UpdateInstallDir(PathBuf::from(current_text))))
    }
    if let TextBoxEvent::Invalid(_) = event {
      ctx.submit_command(Selector::new("druid.builtin.textbox-cancel-editing"))
    }
  }
}

#[derive(Default)]
struct DragWindowController {
  init_pos: Option<Point>,
  //dragging: bool
}

impl<T, W: Widget<T>> Controller<T, W> for DragWindowController {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &druid::Env) {
    match event {
      Event::MouseDown(me) if me.buttons.has_left() => {
        ctx.set_active(true);
        self.init_pos = Some(me.window_pos)
      }
      Event::MouseMove(me) if ctx.is_active() && me.buttons.has_left() => {
        if let Some(init_pos) = self.init_pos {
          let within_window_change = me.window_pos.to_vec2() - init_pos.to_vec2();
          let old_pos = ctx.window().get_position();
          let new_pos = old_pos + within_window_change;
          ctx.window().set_position(new_pos)
        }
      }
      Event::MouseUp(_me) if ctx.is_active() => {
        self.init_pos = None;
        ctx.set_active(false)
      }
      _ => (),
    }
    child.event(ctx, event, data, env)
  }
}

struct UnitController<T, U> {
  lens: Rc<dyn DynLens<T, U>>
}

impl<T: Data, U: Data> UnitController<T, U> {
  fn new(lens: impl Lens<VMParams, Unit> + 'static + Lens<T, U>) -> Self {
    Self {
      lens: Rc::new(lens)
    }
  }
}

impl<W: Widget<VMParams>> Controller<VMParams, W> for UnitController<VMParams, Unit> {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut VMParams, env: &druid::Env) {
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
            let mut menu: Menu<super::App> = Menu::empty();
            for unit in Unit::ALL {
              menu = menu.entry(
                MenuItem::new(unit.to_string()).on_activate({
                  let lens = self.lens.clone();
                  move |_, d: &mut super::App, _| {
                    if let Some(vmparams) = d.settings.vmparams.as_mut() {
                      lens.with_mut(vmparams, |data| *data = unit);
                      if_chain! {
                        if let Some(install_dir) = d.settings.install_dir.clone();
                        let vmparams = vmparams.clone();
                        if let Err(err) = vmparams.save(install_dir);
                        then {
                          eprintln!("{:?}", err)
                        }
                      }
                    }
                  }
                }).enabled(self.lens.with(data, |d| *d != unit))
              )
            }

            ctx.show_context_menu::<super::App>(
              menu,
              ctx.to_window(mouse_event.pos)
            )
          }
          ctx.request_paint();
        }
      }
      _ => {}
    }
    
    child.event(ctx, event, data, env);
  }
}

use std::{path::PathBuf, rc::Rc};

use druid::{
  lens,
  text::ParseFormatter,
  theme,
  widget::{
    Axis, Button, Checkbox, Controller, Flex, Label, SizedBox, TextBox, TextBoxEvent,
    ValidationDelegate, ViewSwitcher, WidgetExt,
  },
  Data, Event, EventCtx, Lens, LensExt, Menu, MenuItem, Selector, Target, Widget,
};
use druid_widget_nursery::{DynLens, WidgetExt as WidgetExtNursery};
use if_chain::if_chain;
use serde::{Deserialize, Serialize};

use self::vmparams::{Unit, VMParams, Value};

use super::util::{
  h2, h3, make_column_pair, make_flex_description_row, make_flex_pair, DragWindowController,
  LabelExt, LoadError, SaveError,
};

pub mod vmparams;

const TRAILING_PADDING: (f64, f64, f64, f64) = (0., 0., 0., 5.);

#[derive(Clone, Data, Lens, Serialize, Deserialize, Default)]
pub struct Settings {
  #[serde(skip)]
  pub dirty: bool,
  #[data(same_fn = "PartialEq::eq")]
  pub install_dir: Option<PathBuf>,
  #[serde(skip)]
  pub install_dir_buf: String,
  #[data(same_fn = "PartialEq::eq")]
  pub last_browsed: Option<PathBuf>,
  pub git_warn: bool,
  pub vmparams_enabled: bool,
  #[serde(skip)]
  pub vmparams: Option<vmparams::VMParams>,
  pub experimental_launch: bool,
  pub experimental_resolution: (u32, u32),
}

impl Settings {
  pub const SELECTOR: Selector<SettingsCommand> = Selector::new("SETTINGS");

  pub fn ui_builder() -> impl Widget<Self> {
    Flex::column()
      .with_child(
        h3("Settings")
          .center()
          .padding(2.)
          .expand_width()
          .background(theme::BACKGROUND_LIGHT)
          .controller(DragWindowController::default()),
      )
      .with_flex_child(
        Flex::column()
          .with_child(Self::install_dir_browser_builder(Axis::Horizontal).padding(TRAILING_PADDING))
          .with_child(
            make_flex_description_row(
              Label::wrapped("Warn when overwriting '.git' folders:"),
              Checkbox::new("").lens(Settings::git_warn),
            )
            .padding(TRAILING_PADDING),
          )
          .with_child(
            make_flex_description_row(
              Label::wrapped("Enable vmparams editing:"),
              Checkbox::new("").lens(Settings::vmparams_enabled),
            )
            .on_change(|_, _old, data, _| {
              if data.vmparams_enabled && data.vmparams.is_none() {
                data.vmparams = data
                  .install_dir
                  .clone()
                  .ok_or(LoadError::NoSuchFile)
                  .and_then(vmparams::VMParams::load)
                  .ok()
              }
            })
            .padding(TRAILING_PADDING),
          )
          .with_child(
            ViewSwitcher::new(
              |data: &Settings, _| data.vmparams_enabled,
              |enabled, data, _| {
                if *enabled && data.vmparams.is_some() {
                  let vmparam_lens = lens::Identity.then(Settings::vmparams).map(
                    |u| u.clone().expect("This has to work..."),
                    |u, data| *u = Some(data),
                  );

                  return Box::new(
                    Flex::column()
                      .with_child(
                        Flex::row()
                          .with_flex_child(
                            Label::new("Minimum RAM:").align_right().expand_width(),
                            3.25,
                          )
                          .with_spacer(5.)
                          .with_flex_child(
                            TextBox::new()
                              .with_formatter(ParseFormatter::new())
                              .lens(VMParams::heap_init.then(Value::amount))
                              .expand_width(),
                            3.,
                          )
                          .with_flex_child(
                            Button::new(|u: &Unit, _env: &druid::Env| u.to_string())
                              .lens(VMParams::heap_init.then(Value::unit))
                              .controller(UnitController::new(
                                VMParams::heap_init.then(Value::unit),
                              ))
                              .expand_width(),
                            0.5,
                          ),
                      )
                      .with_child(
                        Flex::row()
                          .with_flex_child(
                            Label::new("Maximum RAM:").align_right().expand_width(),
                            3.25,
                          )
                          .with_spacer(5.)
                          .with_flex_child(
                            TextBox::new()
                              .with_formatter(ParseFormatter::new())
                              .lens(VMParams::heap_max.then(Value::amount))
                              .expand_width(),
                            3.,
                          )
                          .with_flex_child(
                            Button::new(|u: &Unit, _env: &druid::Env| u.to_string())
                              .lens(VMParams::heap_max.then(Value::unit))
                              .controller(UnitController::new(VMParams::heap_max.then(Value::unit)))
                              .expand_width(),
                            0.5,
                          ),
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
                      }),
                  );
                }
                Box::new(SizedBox::empty())
              },
            )
            .padding(TRAILING_PADDING),
          )
          .with_child(
            make_flex_description_row(
              Label::wrapped("Enable experimental direct launch:"),
              Checkbox::new("").lens(Settings::experimental_launch),
            )
            .padding(TRAILING_PADDING),
          )
          .with_child(
            ViewSwitcher::new(
              |data: &Settings, _| data.experimental_launch,
              |enabled, _, _| {
                if *enabled {
                  let res_lens = lens::Identity.then(Settings::experimental_resolution);

                  return Box::new(
                    Flex::column()
                      .with_child(
                        Flex::row()
                          .with_flex_child(
                            Label::new("Horizontal Resolution:")
                              .align_right()
                              .expand_width(),
                            3.25,
                          )
                          .with_spacer(5.)
                          .with_flex_child(
                            TextBox::new()
                              .with_formatter(ParseFormatter::new())
                              .lens(res_lens.clone().then(lens!((u32, u32), 0)))
                              .expand_width(),
                            3.5,
                          ),
                      )
                      .with_child(
                        Flex::row()
                          .with_flex_child(
                            Label::new("Vertical Resolution:")
                              .align_right()
                              .expand_width(),
                            3.25,
                          )
                          .with_spacer(5.)
                          .with_flex_child(
                            TextBox::new()
                              .with_formatter(ParseFormatter::new())
                              .lens(res_lens.then(lens!((u32, u32), 1)))
                              .expand_width(),
                            3.5,
                          ),
                      ),
                  );
                }
                Box::new(SizedBox::empty())
              },
            )
            .padding(TRAILING_PADDING),
          )
          .padding((10., 10.))
          .expand(),
        1.,
      )
      .with_child(
        Flex::row()
          .with_child(
            Button::new("Close")
              .on_click(|ctx, _, _| ctx.submit_command(druid::commands::CLOSE_WINDOW)),
          )
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::End)
          .main_axis_alignment(druid::widget::MainAxisAlignment::End)
          .expand_width(),
      )
      .expand_height()
  }

  pub fn install_dir_browser_builder(axis: Axis) -> Flex<Self> {
    let input = TextBox::multiline()
      .with_line_wrapping(true)
      .with_formatter(ParseFormatter::new())
      .delegate(InstallDirDelegate {})
      .lens(lens!(Settings, install_dir_buf));

    match axis {
      Axis::Horizontal => make_flex_pair(
        Label::wrapped("Starsector Install Directory:"),
        1.,
        Flex::for_axis(axis)
          .with_flex_child(input.expand_width(), 1.)
          .with_child(Button::new("Browse...").on_click(|ctx, _, _| {
            ctx.submit_command(
              Selector::new("druid.builtin.textbox-cancel-editing").to(Target::Global),
            );
            ctx.submit_command(
              Settings::SELECTOR
                .with(SettingsCommand::SelectInstallDir)
                .to(Target::Global),
            )
          })),
        1.5,
        axis,
      ),
      Axis::Vertical => make_column_pair(
        h2("Starsector Install Directory:"),
        Flex::for_axis(axis)
          .with_child(input.expand_width())
          .with_child(Button::new("Browse...").on_click(|ctx, _, _| {
            ctx.submit_command(
              Selector::new("druid.builtin.textbox-cancel-editing").to(Target::Global),
            );
            ctx.submit_command(
              Settings::SELECTOR
                .with(SettingsCommand::SelectInstallDir)
                .to(Target::Global),
            )
          }))
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::End),
      ),
    }
  }

  pub fn path(try_make: bool) -> PathBuf {
    use directories::ProjectDirs;
    use std::fs;

    if let Some(proj_dirs) = ProjectDirs::from("org", "laird", "Starsector Mod Manager") {
      if proj_dirs.config_dir().exists()
        || (try_make && fs::create_dir_all(proj_dirs.config_dir()).is_ok())
      {
        return proj_dirs.config_dir().to_path_buf().join("config.json");
      }
    };
    PathBuf::from(r"./config.json")
  }

  pub fn load() -> Result<Settings, LoadError> {
    use std::{fs, io::Read};

    let mut config_file =
      fs::File::open(Settings::path(false)).map_err(|_| LoadError::NoSuchFile)?;

    let mut config_string = String::new();
    config_file
      .read_to_string(&mut config_string)
      .map_err(|_| LoadError::ReadError)?;

    serde_json::from_str::<Settings>(&config_string)
      .map_err(|_| LoadError::FormatError)
      .map(|mut settings| {
        settings.dirty = true;
        settings
      })
  }

  pub fn save(&self) -> Result<(), SaveError> {
    use std::{fs, io::Write};

    let json = serde_json::to_string_pretty(&self).map_err(|_| SaveError::Format)?;

    let mut file = fs::File::create(Settings::path(true)).map_err(|_| SaveError::File)?;

    file
      .write_all(json.as_bytes())
      .map_err(|_| SaveError::Write)
  }
}

pub enum SettingsCommand {
  UpdateInstallDir(PathBuf),
  SelectInstallDir,
}

struct InstallDirDelegate {}

impl ValidationDelegate for InstallDirDelegate {
  fn event(&mut self, ctx: &mut druid::EventCtx, event: TextBoxEvent, current_text: &str) {
    if let TextBoxEvent::Complete | TextBoxEvent::Changed = event {
      ctx.submit_command(
        Settings::SELECTOR.with(SettingsCommand::UpdateInstallDir(PathBuf::from(
          current_text,
        ))),
      )
    }
    if let TextBoxEvent::Invalid(_) = event {
      ctx.submit_command(Selector::new("druid.builtin.textbox-cancel-editing"))
    }
  }
}

struct UnitController<T, U> {
  lens: Rc<dyn DynLens<T, U>>,
}

impl<T: Data, U: Data> UnitController<T, U> {
  fn new(lens: impl Lens<VMParams, Unit> + 'static + Lens<T, U>) -> Self {
    Self {
      lens: Rc::new(lens),
    }
  }
}

impl<W: Widget<VMParams>> Controller<VMParams, W> for UnitController<VMParams, Unit> {
  fn event(
    &mut self,
    child: &mut W,
    ctx: &mut EventCtx,
    event: &Event,
    data: &mut VMParams,
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
            let mut menu: Menu<super::App> = Menu::empty();
            for unit in Unit::ALL {
              menu = menu.entry(
                MenuItem::new(unit.to_string())
                  .on_activate({
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
                  })
                  .enabled(self.lens.with(data, |d| *d != unit)),
              )
            }

            ctx.show_context_menu::<super::App>(menu, ctx.to_window(mouse_event.pos))
          }
          ctx.request_paint();
        }
      }
      _ => {}
    }

    child.event(ctx, event, data, env);
  }
}

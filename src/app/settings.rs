use std::{path::PathBuf, rc::Rc};

use druid::{
  im::Vector,
  lens,
  text::ParseFormatter,
  theme,
  widget::{
    Axis, Button, Checkbox, Controller, Either, Flex, Label, Maybe, Painter, SizedBox, TextBox,
    TextBoxEvent, ValidationDelegate, ViewSwitcher, WidgetExt,
  },
  Data, Event, EventCtx, Lens, LensExt, Menu, MenuItem, RenderContext, Selector, Widget,
  WindowConfig,
};
use druid_widget_nursery::{material_icons::Icon, DynLens, WidgetExt as WidgetExtNursery};
use if_chain::if_chain;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use tap::{Pipe, Tap};

use crate::{app::PROJECT, patch::click::Click};

use self::{
  jre::{revert, Flavour},
  vmparams::{Unit, VMParams, Value},
};

use super::{
  controllers::HoverController,
  mod_list::headings::{Header, Heading},
  modal::Modal,
  util::{
    bold_text, button_painter, default_true, h2, icons::*, make_column_pair, make_flex_pair,
    make_flex_settings_row, Button2, Card, CommandExt, LabelExt, LoadError, SaveError,
  },
  App,
};

pub mod jre;
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
  #[serde(default = "default_true")]
  pub hide_webview_on_conflict: bool,
  #[serde(default = "default_true")]
  pub open_forum_link_in_webview: bool,
  #[serde(skip)]
  show_column_editor: bool,
  #[serde(default = "default_headers")]
  #[data(same_fn = "PartialEq::eq")]
  pub headings: Vector<Heading>,
  #[serde(skip)]
  show_jre_swapper: bool,
  #[serde(skip)]
  jre_swap_in_progress: bool,
  jre_managed_mode: bool,
  pub show_auto_update_for_discrepancy: bool,
}

fn default_headers() -> Vector<Heading> {
  Header::TITLES.to_vec().into()
}

impl Settings {
  pub const SELECTOR: Selector<SettingsCommand> = Selector::new("SETTINGS");

  pub fn new() -> Self {
    Self {
      hide_webview_on_conflict: true,
      open_forum_link_in_webview: true,
      headings: default_headers(),
      ..Default::default()
    }
  }

  pub fn ui_builder() -> impl Widget<Self> {
    Modal::new("Settings")
      .with_content(
        Flex::column()
          .with_child(Self::install_dir_browser_builder(Axis::Horizontal).padding(TRAILING_PADDING))
          .with_child(
            make_flex_settings_row(
              Checkbox::new("").lens(Settings::git_warn),
              Label::wrapped("Warn when overwriting '.git' folders"),
            )
            .padding(TRAILING_PADDING),
          )
          .with_child(
            make_flex_settings_row(
              Checkbox::new("").lens(Settings::hide_webview_on_conflict),
              Label::wrapped("Minimize browser when installation encounters conflict"),
            )
            .padding(TRAILING_PADDING),
          )
          .with_child(
            make_flex_settings_row(
              Checkbox::new("").lens(Settings::open_forum_link_in_webview),
              Label::wrapped("Use bundled browser when opening forum links")
                .stack_tooltip("This allows installing mods directly from links in forum posts")
                .with_crosshair(true),
            )
            .padding(TRAILING_PADDING),
          )
          .with_child(
            make_flex_settings_row(
              Checkbox::new("").lens(Settings::show_auto_update_for_discrepancy),
              Flex::column()
                .with_child(Label::wrapped("Show automatic updates even for mods that have a version discrepancy"))
                .with_child(Label::wrapped("(Recommended Off)"))
                .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            )
            .padding(TRAILING_PADDING)
          )
          .with_child(
            make_flex_settings_row(
              SizedBox::empty(),
              Button::from_label(Label::wrapped("Edit columns")).on_click(
                |ctx, data: &mut Settings, env| {
                  let modal = Modal::<Settings>::new("Column Editor")
                    .with_content(
                      ViewSwitcher::new(
                        |headings: &Vector<Heading>, _| headings.clone(),
                        |_, headings, _| {
                          Flex::row()
                            .tap_mut(|column| {
                              for (idx, heading) in headings.iter().cloned().enumerate() {
                                column.add_flex_child(
                                  Flex::row()
                                    .with_default_spacer()
                                    .with_child(
                                      Icon::new(ARROW_LEFT)
                                        .background(button_painter())
                                        .controller(HoverController)
                                        .on_click(move |ctx, data: &mut Vector<Heading>, _| {
                                          data.swap(idx - 1, idx);
                                          ctx.submit_command_global(
                                            Header::SWAP_HEADINGS.with((idx - 1, idx)),
                                          )
                                        })
                                        .pipe(|icon| {
                                          if idx == 0 {
                                            icon.disabled_if(|_, _| true).boxed()
                                          } else {
                                            icon.boxed()
                                          }
                                        }),
                                    )
                                    .with_flex_child(
                                      Label::wrapped(<&str>::from(heading))
                                        .with_text_alignment(druid::TextAlignment::Center)
                                        .expand_width(),
                                      1.,
                                    )
                                    .with_child(
                                      Icon::new(CLOSE)
                                        .on_click(move |ctx, data: &mut Vector<Heading>, _| {
                                          if data.len() > 1 {
                                            data.retain(|existing| existing != &heading);
                                            ctx.submit_command_global(
                                              Header::REMOVE_HEADING.with(heading),
                                            );
                                          }
                                        })
                                        .pipe(|icon| {
                                          if headings.len() <= 1 {
                                            icon.disabled_if(|_, _| true).boxed()
                                          } else {
                                            icon.boxed()
                                          }
                                        })
                                        .controller(HoverController),
                                    )
                                    .with_child(
                                      Icon::new(ARROW_RIGHT)
                                        .background(button_painter())
                                        .controller(HoverController)
                                        .on_click(move |ctx, data: &mut Vector<Heading>, _| {
                                          data.swap(idx, idx + 1);
                                          ctx.submit_command_global(
                                            Header::SWAP_HEADINGS.with((idx, idx + 1)),
                                          );
                                        })
                                        .pipe(|icon| {
                                          if idx == headings.len() - 1 {
                                            icon.disabled_if(|_, _| true).boxed()
                                          } else {
                                            icon.boxed()
                                          }
                                        }),
                                    )
                                    .with_default_spacer()
                                    .padding((0., 5., 0., 5.))
                                    .background(Painter::new(|ctx, _, env| {
                                      let border_rect = ctx.size().to_rect().inset(-1.5);
                                      if ctx.is_hot() {
                                        ctx.stroke(
                                          border_rect,
                                          &env.get(druid::theme::BORDER_LIGHT),
                                          3.,
                                        )
                                      }
                                    }))
                                    .on_click(|_, _, _| {}),
                                  1.,
                                )
                              }
                            })
                            .boxed()
                        },
                      )
                      .lens(Settings::headings)
                      .boxed(),
                    )
                    .with_content(
                      Flex::row()
                        .with_flex_spacer(1.)
                        .with_flex_child(
                          Button::new("Add new column")
                            .controller(Click::new(|ctx, mouse_event, data: &mut Settings, _| {
                              let mut menu: Menu<super::App> = Menu::empty();
                              for heading in Heading::iter().filter(|heading| {
                                !matches!(heading, Heading::Score | Heading::Enabled)
                                  && !data.headings.contains(heading)
                              }) {
                                menu = menu.entry(MenuItem::new(<&str>::from(heading)).on_activate(
                                  move |ctx, data: &mut App, _| {
                                    data.settings.headings.push_back(heading);
                                    ctx.submit_command(
                                      Header::ADD_HEADING.with(heading).to(druid::Target::Global),
                                    )
                                  },
                                ))
                              }

                              ctx.show_context_menu::<super::App>(
                                menu,
                                ctx.to_window(mouse_event.pos),
                              )
                            }))
                            .expand_width(),
                          2.,
                        )
                        .with_flex_spacer(1.)
                        .boxed(),
                    )
                    .with_close()
                    .build();

                  ctx.new_sub_window(
                    WindowConfig::default()
                      .window_size((1200., 200.))
                      .show_titlebar(false),
                    modal,
                    data.clone(),
                    env.clone(),
                  );
                },
              ),
            )
            .padding(TRAILING_PADDING),
          )
          .with_child(
            ViewSwitcher::new(
              |data: &Settings, _| data.show_column_editor,
              |_, data, _| {
                if data.show_column_editor {
                  ViewSwitcher::new(
                    |headings: &Vector<Heading>, _| headings.clone(),
                    |_, headings, _| {
                      Flex::row()
                        .tap_mut(|row| {
                          for (idx, heading) in headings.iter().enumerate() {
                            row.add_flex_child(
                              Flex::row()
                                .with_default_spacer()
                                .with_child(
                                  Icon::new(ARROW_LEFT)
                                    .background(button_painter())
                                    .controller(HoverController)
                                    .on_click(move |ctx, data: &mut Vector<Heading>, _| {
                                      data.swap(idx - 1, idx);
                                      ctx.submit_command_global(
                                        Header::SWAP_HEADINGS.with((idx - 1, idx)),
                                      )
                                    })
                                    .pipe(|icon| {
                                      if idx == 0 {
                                        icon.disabled_if(|_, _| true).boxed()
                                      } else {
                                        icon.boxed()
                                      }
                                    }),
                                )
                                .with_flex_child(
                                  Label::wrapped(<&str>::from(*heading))
                                    .with_text_alignment(druid::TextAlignment::Center)
                                    .expand_width(),
                                  1.,
                                )
                                .with_child(
                                  Icon::new(ARROW_RIGHT)
                                    .background(button_painter())
                                    .controller(HoverController)
                                    .on_click(move |ctx, data: &mut Vector<Heading>, _| {
                                      data.swap(idx, idx + 1);
                                      ctx.submit_command_global(
                                        Header::SWAP_HEADINGS.with((idx, idx + 1)),
                                      );
                                    })
                                    .pipe(|icon| {
                                      if idx == headings.len() - 1 {
                                        icon.disabled_if(|_, _| true).boxed()
                                      } else {
                                        icon.boxed()
                                      }
                                    }),
                                )
                                .with_default_spacer()
                                .padding((0., 5., 0., 5.))
                                .background(Painter::new(|ctx, _, env| {
                                  let border_rect = ctx.size().to_rect().inset(-1.5);
                                  if ctx.is_hot() {
                                    ctx.stroke(
                                      border_rect,
                                      &env.get(druid::theme::BORDER_LIGHT),
                                      3.,
                                    )
                                  }
                                }))
                                .on_click(|_, _, _| {}),
                              1.,
                            )
                          }
                        })
                        .boxed()
                    },
                  )
                  .lens(Settings::headings)
                  .boxed()
                } else {
                  SizedBox::empty().boxed()
                }
              },
            )
            .padding(TRAILING_PADDING),
          )
          .with_child(
            make_flex_settings_row(
              Checkbox::new("").lens(Settings::vmparams_enabled),
              Label::wrapped("Enable vmparams editing"),
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
            Either::new(
              |data: &Settings, _| data.vmparams_enabled && data.vmparams.is_some(),
              Maybe::or_empty(|| {
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
                          .update_data_while_editing(true)
                          .lens(VMParams::heap_init.then(Value::amount))
                          .expand_width(),
                        3.,
                      )
                      .with_flex_child(
                        Button::new(|u: &Unit, _env: &druid::Env| u.to_string())
                          .lens(VMParams::heap_init.then(Value::unit))
                          .controller(UnitController::new(VMParams::heap_init.then(Value::unit)))
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
                          .update_data_while_editing(true)
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
              })
              .lens(Settings::vmparams)
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
              SizedBox::empty(),
            )
            .padding(TRAILING_PADDING),
          )
          .with_child(
            make_flex_settings_row(
              Either::new(
                |data, _| *data,
                Icon::new(ARROW_DROP_DOWN),
                Icon::new(ARROW_RIGHT),
              )
              .padding((-5., 0., 0., 0.)),
              Label::new("Open JRE Switcher"),
            )
            .controller(HoverController)
            .on_click(|_, data, _| *data = !*data)
            .lens(Settings::show_jre_swapper)
            .padding(TRAILING_PADDING.tap_mut(|padding| padding.2 = -5.)),
          )
          .with_child(
            Either::new(
              |data: &Settings, _| data.show_jre_swapper,
              make_flex_settings_row(
                SizedBox::empty(),
                Flex::column()
                  .with_child(
                    Flex::row()
                      .with_flex_child(
                        Card::new(
                          Flex::column()
                            .with_child(h2("Wisp's Archived JRE"))
                            .with_child(bold_text(
                              "JRE 8v271",
                              theme::TEXT_SIZE_NORMAL,
                              druid::FontWeight::SEMI_BOLD,
                              druid::theme::TEXT_COLOR,
                            ))
                            .with_child(bold_text(
                              "(RECOMMENDED)",
                              theme::TEXT_SIZE_NORMAL,
                              druid::FontWeight::MEDIUM,
                              druid::Color::GREEN,
                            ))
                            .with_spacer(5.)
                            .with_child(
                              Button2::new(Label::new("Install").padding((10., 0.))).on_click(
                                |ctx, data: &mut Settings, _| {
                                  data.jre_swap_in_progress = true;
                                  tokio::runtime::Handle::current().spawn(Flavour::Wisp.swap(
                                    ctx.get_external_handle(),
                                    data.install_dir.as_ref().unwrap().clone(),
                                    data.jre_managed_mode
                                  ));
                                },
                              ),
                            )
                            .main_axis_alignment(druid::widget::MainAxisAlignment::Center),
                        )
                        .expand_width(),
                        1.,
                      )
                      .with_flex_child(
                        Card::new(
                          Flex::column()
                            .with_child(h2("Amazon Coretto"))
                            .with_child(bold_text(
                              "JRE 8v272 (10.3)",
                              theme::TEXT_SIZE_NORMAL,
                              druid::FontWeight::SEMI_BOLD,
                              druid::theme::TEXT_COLOR,
                            ))
                            .with_child(bold_text(
                              "(UNSUPPORTED)",
                              theme::TEXT_SIZE_NORMAL,
                              druid::FontWeight::MEDIUM,
                              druid::Color::MAROON,
                            ))
                            .with_spacer(5.)
                            .with_child(
                              Button2::new(Label::new("Install").padding((10., 0.))).on_click(
                                |ctx, data: &mut Settings, _| {
                                  data.jre_swap_in_progress = true;
                                  tokio::runtime::Handle::current().spawn(Flavour::Coretto.swap(
                                    ctx.get_external_handle(),
                                    data.install_dir.as_ref().unwrap().clone(),
                                    data.jre_managed_mode
                                  ));
                                },
                              ),
                            )
                            .main_axis_alignment(druid::widget::MainAxisAlignment::Center),
                        )
                        .expand_width(),
                        1.,
                      )
                      .with_flex_child(
                        Card::new(
                          Flex::column()
                            .with_child(h2("OpenJDK Hotspot"))
                            .with_child(bold_text(
                              "JRE 8v272 (b10)",
                              theme::TEXT_SIZE_NORMAL,
                              druid::FontWeight::SEMI_BOLD,
                              druid::theme::TEXT_COLOR,
                            ))
                            .with_child(bold_text(
                              "(UNSUPPORTED)",
                              theme::TEXT_SIZE_NORMAL,
                              druid::FontWeight::MEDIUM,
                              druid::Color::MAROON,
                            ))
                            .with_spacer(5.)
                            .with_child(
                              Button2::new(Label::new("Install").padding((10., 0.))).on_click(
                                |ctx, data: &mut Settings, _| {
                                  data.jre_swap_in_progress = true;
                                  tokio::runtime::Handle::current().spawn(Flavour::Hotspot.swap(
                                    ctx.get_external_handle(),
                                    data.install_dir.as_ref().unwrap().clone(),
                                    data.jre_managed_mode
                                  ));
                                },
                              ),
                            )
                            .main_axis_alignment(druid::widget::MainAxisAlignment::Center),
                        )
                        .expand_width(),
                        1.,
                      )
                      .expand_width(),
                  )
                  .with_child(
                    Button2::new(Label::new("Revert to Vanilla/Stock JRE 7").padding((10., 0.)))
                      .on_click(|ctx, data: &mut Settings, _| {
                        data.jre_swap_in_progress = true;
                        tokio::runtime::Handle::current().spawn(revert(
                          ctx.get_external_handle(),
                          data.install_dir.as_ref().unwrap().clone(),
                        ));
                      })
                      .align_left()
                      .padding(TRAILING_PADDING)
                      .expand_width(),
                  )
                  .with_child(make_flex_settings_row(
                    Checkbox::new("").lens(Settings::jre_managed_mode),
                    Label::wrapped("Enable 'Managed' mode.")
                  ))
                  .with_child(make_flex_settings_row(
                    SizedBox::empty(),
                    Label::wrapped("\
                      'Managed' mode stores JRE updates in a MOSS managed data folder, \
                      keeping your Starsector install folder clutter free.\n\
                      Unfortunately, if you're on Windows, MOSS must be run with administrator privileges for this mode to work.\
                    ")
                  ))
                  .disabled_if(|data: &Settings, _| data.install_dir.is_none())
                  .on_command(jre::SWAP_COMPLETE, |_, _, data| {
                    data.jre_swap_in_progress = false
                  })
                  .expand_width(),
              ),
              SizedBox::empty(),
            )
            .padding(TRAILING_PADDING),
          )
          .with_child(
            make_flex_settings_row(
              Checkbox::new("").lens(Settings::experimental_launch),
              Label::wrapped("Enable experimental direct launch"),
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
                              .update_data_while_editing(true)
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
                              .update_data_while_editing(true)
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
          .expand()
          .on_change(|_, _old, data, _| {
            if let Err(err) = data.save() {
              eprintln!("{:?}", err)
            }
          })
          .on_command(Header::ADD_HEADING, |_, _heading, settings| {
            if let Err(err) = settings.save() {
              eprintln!("{:?}", err)
            }
          })
          .boxed(),
      )
      .with_close()
      .build()
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
          .with_child(
            Button::new("Browse...")
              .controller(HoverController)
              .on_click(|ctx, _, _| {
                ctx.submit_command_global(Selector::new("druid.builtin.textbox-cancel-editing"));
                ctx
                  .submit_command_global(Settings::SELECTOR.with(SettingsCommand::SelectInstallDir))
              }),
          ),
        1.5,
        axis,
      ),
      Axis::Vertical => make_column_pair(
        h2("Starsector Install Directory:"),
        Flex::for_axis(axis)
          .with_child(input.expand_width())
          .with_child(
            Button::new("Browse...")
              .controller(HoverController)
              .on_click(|ctx, _, _| {
                ctx.submit_command_global(Selector::new("druid.builtin.textbox-cancel-editing"));
                ctx
                  .submit_command_global(Settings::SELECTOR.with(SettingsCommand::SelectInstallDir))
              }),
          )
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::End),
      ),
    }
  }

  pub fn path(try_make: bool) -> PathBuf {
    use std::fs;

    if PROJECT.config_dir().exists()
      || (try_make && fs::create_dir_all(PROJECT.config_dir()).is_ok())
    {
      return PROJECT.config_dir().to_path_buf().join("config.json");
    }
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
      let path = PathBuf::from(current_text);
      if path.exists() {
        ctx.submit_command(Settings::SELECTOR.with(SettingsCommand::UpdateInstallDir(
          PathBuf::from(current_text),
        )))
      }
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
            for unit in Unit::iter() {
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

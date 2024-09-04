use std::{convert::identity, ops::Not, path::PathBuf};

use druid::{
  im::Vector,
  lens::Map,
  text::ParseFormatter,
  widget::{
    Checkbox, Flex, Label, Painter, SizedBox, TextBox, TextBoxEvent, ValidationDelegate, WidgetExt,
  },
  Data, Insets, Key, Lens, LensExt, Selector, Widget,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as _};
use extend::ext;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;

use super::{
  controllers::{HoverController, Rotated},
  mod_list::headings::{Header, Heading},
  tools::vmparams::VMParams,
  util::{
    bolded, button_painter, default_true, h2_fixed, hoverable_text, icons::*, lensed_bold,
    CommandExt, LabelExt, LoadError, SaveError, Tap, WidgetExtEx, WithHoverState,
  },
  App,
};
use crate::{
  app::PROJECT,
  nav_bar::Nav,
  theme::{Theme, Themes},
  widgets::{
    card::Card, card_button::CardButton, root_stack::RootStack, wrapped_table::WrappedTable,
  },
};

mod theme_editor;

pub use theme_editor::*;

#[derive(Clone, Data, Lens, Serialize, Deserialize, Default, Debug)]
pub struct Settings {
  #[serde(skip)]
  pub dirty: bool,
  #[data(eq)]
  pub install_dir: Option<PathBuf>,
  #[serde(skip)]
  pub install_dir_buf: String,
  #[data(eq)]
  pub last_browsed: Option<PathBuf>,
  pub git_warn: bool,
  #[serde(default = "default_true")]
  pub experimental_launch: bool,
  pub experimental_resolution: Option<(u32, u32)>,
  #[serde(default = "default_true")]
  pub hide_webview_on_conflict: bool,
  #[serde(default = "default_true")]
  pub open_forum_link_in_webview: bool,
  #[serde(default = "default_headers")]
  pub headings: Vector<Heading>,
  #[serde(alias = "show_auto_update_for_discrepancy")]
  pub show_discrepancies: bool,
  #[serde(default)]
  pub theme: Themes,
  #[serde(skip)]
  pub(crate) vmparams: Option<VMParams>,
  pub vmparams_linked: bool,

  #[serde(default = "default_true")]
  pub show_duplicate_warnings: bool,
  #[serde(default)]
  pub custom_theme: Theme,
  #[serde(default)]
  pub jre_23: bool,
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
      show_duplicate_warnings: true,
      headings: default_headers(),
      ..Default::default()
    }
  }

  pub fn view() -> impl Widget<Self> {
    Card::new(
      Flex::column()
        .with_child(h2_fixed("Starsector installation directory:"))
        .with_child(
          Flex::row()
            .with_flex_child(
              TextBox::multiline()
                .with_line_wrapping(true)
                .with_formatter(ParseFormatter::new())
                .delegate(InstallDirDelegate)
                .lens(Settings::install_dir_buf)
                .expand_width(),
              1.,
            )
            .with_child(
              CardButton::button_with(
                |_| CardButton::button_text("Browse...").padding((4.0, 0.0)),
                Card::builder()
                  .with_background(druid::theme::BUTTON_DARK)
                  .with_insets((0.0, 10.0))
                  .with_border(0.5, druid::theme::BORDER_DARK),
              )
              .controller(HoverController::default())
              .on_click(|ctx, _, _| {
                ctx.submit_command_global(Selector::new("druid.builtin.textbox-cancel-editing"));
                ctx
                  .submit_command_global(Settings::SELECTOR.with(SettingsCommand::SelectInstallDir))
              }),
            ),
        )
        .with_default_spacer()
        .with_child(
          Flex::column()
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            .with_child(h2_fixed("Warn when overwriting '.git' folders:"))
            .with_child(
              Checkbox::from_label(Label::wrapped(
                "Aimed at developers. If a mod folder is an active Git project this option will \
                 warn you if it would be overwritten or deleted (Unimplemented)",
              ))
              .lens(Settings::git_warn),
            )
            .mask_default()
            .with_mask(SizedBox::empty()),
        )
        .with_default_spacer()
        .with_child(h2_fixed("Use in-app browser to open links:"))
        .with_child(
          Checkbox::from_label(Label::wrapped(
            "Uses an embedded browser when enabled. If disabled links will open in your system \
             default web browser.",
          ))
          .lens(Settings::open_forum_link_in_webview),
        )
        .with_default_spacer()
        .with_child(h2_fixed(
          "Show a warning/automatic updates for mods that have a version discrepancy",
        ))
        .with_child(
          Checkbox::from_label(Label::wrapped(
            "Indicates a mod has an update even when the installed version is a higher/more \
             recent version than is advertised by the author online.\n(Recommended Off)",
          ))
          .lens(Settings::show_discrepancies),
        )
        .with_default_spacer()
        .with_child(h2_fixed(
          "Show a warning when more than one copy of a mod is installed:",
        ))
        .with_child(
          Checkbox::from_label(Label::wrapped(
            "When more than one copy of a mod is installed at the same time it is random which \
             version is actually loaded by the game.",
          ))
          .lens(Settings::show_duplicate_warnings),
        )
        .with_default_spacer()
        .with_child(h2_fixed("Edit columns:"))
        .with_child(Self::headings_editor())
        .with_default_spacer()
        .with_child(h2_fixed("Theme:"))
        .with_spacer(5.0)
        .with_child(
          Card::builder()
            .with_insets(0.1)
            .with_corner_radius(0.0)
            .with_shadow_length(0.0)
            .with_shadow_increase(0.0)
            .with_border(1.0, druid::Color::BLACK)
            .stacked_button(
              |_| Self::theme_picker_heading(true, 7.0),
              |_| Self::theme_picker_expanded(Themes::iter()),
              CardButton::stack_none(),
              150.0,
            )
            .lens(Settings::theme),
        )
        .with_spacer(5.0)
        .with_child(
          hoverable_text(Option::<druid::Color>::None)
            .constant("Edit Custom Theme".to_owned())
            .on_click(|ctx, _, _| {
              ctx.submit_command(Nav::NAV_SELECTOR.with(crate::nav_bar::NavLabel::ThemeEditor))
            })
            .with_hover_state(false)
            .empty_if_not(|data, _| data == &Themes::Custom)
            .lens(Settings::theme),
        )
        .with_default_spacer()
        .with_child(
          hoverable_text(Option::<druid::Color>::None)
            .constant("Open config.json".to_owned())
            .with_hover_state(false)
            .stack_tooltip_custom(Card::new(
              bolded("Requires application restart for changes to apply.").padding((7.0, 0.0)),
            ))
            .with_offset((10.0, 10.0))
            .on_click(|_, _, _| {
              let _ = opener::open(Settings::path(false));
            })
            .align_right()
            .expand_width(),
        )
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
        .must_fill_main_axis(true)
        .scroll()
        .vertical()
        .padding((12.0, 0.0))
        .expand()
        .on_change(Self::save_on_change)
        .on_command(Header::ADD_HEADING, Self::save_on_command)
        .on_command(Header::REMOVE_HEADING, Self::save_on_command)
        .on_command(Header::SWAP_HEADINGS, Self::save_on_command),
    )
  }

  fn headings_editor() -> impl Widget<Self> {
    WrappedTable::new(250.0, |_, _, map_id| {
      let map_id = std::rc::Rc::new(map_id);
      Card::builder()
        .with_shadow_length(6.)
        .with_background(druid::theme::BACKGROUND_DARK)
        .build(
          Flex::row()
            .with_default_spacer()
            .with_child({
              Icon::new(*ARROW_LEFT)
                .background(button_painter())
                .controller(HoverController::default())
                .on_click({
                  let map_id = map_id.clone();
                  move |ctx, data: &mut Vector<Heading>, env| {
                    let idx = map_id(env);
                    data.swap(idx - 1, idx);
                    ctx.submit_command(Header::SWAP_HEADINGS.with((idx - 1, idx)))
                  }
                })
                .disabled_if({
                  let map_id = map_id.clone();
                  move |_, env| map_id(env) == 0
                })
                .invisible_if({
                  let map_id = map_id.clone();
                  move |_, env| map_id(env) == 0
                })
            })
            .with_flex_child(
              Label::wrapped_func({
                let map_id = map_id.clone();
                move |data: &Vector<Heading>, env| {
                  let idx = map_id(env);
                  format!("{}. {}", idx + 1, data.get(idx).unwrap_or(&Heading::Score))
                }
              })
              .with_text_alignment(druid::TextAlignment::Center)
              .expand_width()
              .padding((5.0, 0.0)),
              1.,
            )
            .with_child(
              Icon::new(*CLOSE)
                .on_click({
                  let map_id = map_id.clone();
                  move |ctx, data: &mut Vector<Heading>, env| {
                    let heading = data[map_id(env)];
                    if data.len() > 1 {
                      data.retain(|existing| existing != &heading);
                      ctx.submit_command_global(Header::REMOVE_HEADING.with(heading));
                    }
                  }
                })
                .disabled_if(|data, _| data.len() <= 1)
                .controller(HoverController::default()),
            )
            .with_child({
              Icon::new(*ARROW_RIGHT)
                .background(button_painter())
                .controller(HoverController::default())
                .on_click({
                  let map_id = map_id.clone();
                  move |ctx, data: &mut Vector<Heading>, env| {
                    let idx = map_id(env);
                    data.swap(idx, idx + 1);
                    ctx.submit_command(Header::SWAP_HEADINGS.with((idx, idx + 1)))
                  }
                })
                .disabled_if({
                  let map_id = map_id.clone();
                  move |data, env| map_id(env) == data.len() - 1
                })
                .invisible_if({
                  let map_id = map_id.clone();
                  move |data, env| map_id(env) == data.len() - 1
                })
            })
            .with_default_spacer()
            .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
            .must_fill_main_axis(true)
            .padding((0., 5., 0., 5.))
            .boxed(),
        )
        .padding(2.)
        .lens(Map::new(
          |data: &Vector<Option<Heading>>| data.iter().cloned().filter_map(identity).collect(),
          |data, opts: Vector<Heading>| {
            let incomplete = !Heading::complete(&opts);
            *data = opts
              .into_iter()
              .map(Some)
              .chain(incomplete.then_some(None))
              .collect::<Vector<Option<Heading>>>()
          },
        ))
        .else_if(
          {
            let map_id = map_id.clone();
            move |data, env| data[map_id(env)].is_none()
          },
          {
            Card::builder()
              .with_insets((-4., 18.))
              .with_shadow_length(6.0)
              .with_shadow_increase(2.0)
              .with_background(druid::theme::BACKGROUND_DARK)
              .stacked_button(
                |_| {
                  Flex::row()
                    .with_default_spacer()
                    .with_child(Icon::new(*ARROW_RIGHT).disabled().invisible())
                    .with_flex_child(
                      Label::wrapped("Add Column")
                        .with_text_alignment(druid::TextAlignment::Center)
                        .expand_width()
                        .padding((5.0, 0.0)),
                      1.,
                    )
                    .with_child(Icon::new(*ADD_CIRCLE_OUTLINE))
                    .with_child(Icon::new(*ARROW_RIGHT).disabled().invisible())
                    .with_default_spacer()
                    .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
                    .must_fill_main_axis(true)
                    .padding((0., 5., 0., 5.))
                },
                Self::add_column_dropdown,
                CardButton::stack_none(),
                250.0,
              )
          },
        )
        .boxed()
    })
    .lens(Settings::headings.map(
      |headings| {
        headings
          .iter()
          .cloned()
          .map(Some)
          .chain(Heading::complete(headings).not().then_some(None))
          .collect::<Vector<Option<Heading>>>()
      },
      |headings, synth| *headings = synth.into_iter().filter_map(identity).collect(),
    ))
  }

  fn add_column_dropdown(_: bool) -> Box<dyn Widget<super::App>> {
    #[ext]
    impl<T: Data> Flex<T> {
      fn padded_row() -> Flex<T> {
        Flex::row()
          .must_fill_main_axis(true)
          .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
          .with_default_spacer()
          .with_child(Icon::new(*ARROW_RIGHT).disabled().invisible())
      }

      fn with_content(self, widget: impl Widget<T> + 'static) -> impl Widget<T> {
        self
          .with_flex_child(widget.expand_width().padding((5., 17.)), 1.)
          .with_child(Icon::new(*ARROW_RIGHT).disabled().invisible())
          .with_default_spacer()
          .lens(druid::lens!((T, bool), 0))
          .background(Painter::new(|ctx, data: &(T, bool), _| {
            use druid::RenderContext;

            if data.1 {
              let path = ctx.size().to_rect().inset(-0.5).to_rounded_rect(3.);

              ctx.stroke(path, &druid::Color::BLACK, 1.)
            }
          }))
          .with_hover_state(false)
      }
    }

    let mut column = Flex::column()
      .with_child(
        Card::builder()
          .with_insets(0.0)
          .with_shadow_length(0.0)
          .with_background(druid::theme::BACKGROUND_DARK)
          .build(
            Flex::row()
              .with_default_spacer()
              .with_child(Icon::new(*ARROW_RIGHT).disabled().invisible())
              .with_flex_child(
                Label::wrapped("Add Column")
                  .with_text_alignment(druid::TextAlignment::Center)
                  .expand_width()
                  .padding((5., 17.)),
                1.,
              )
              .with_child(Icon::new(*ADD_CIRCLE))
              .with_child(Icon::new(*ARROW_RIGHT).disabled().invisible())
              .with_default_spacer()
              .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
              .must_fill_main_axis(true),
          )
          .padding((0., -9., 0., 0.)),
      )
      .with_spacer(10.);

    for heading in Heading::iter().filter(|h| !matches!(h, Heading::Enabled | Heading::Score)) {
      column.add_child(
        Flex::padded_row()
          .with_content(
            Label::wrapped(heading.to_string()).with_text_alignment(druid::TextAlignment::Center),
          )
          .on_click(move |ctx, data: &mut Vector<Heading>, _| {
            data.push_back(heading);
            ctx.submit_command(Header::ADD_HEADING.with(heading))
          })
          .disabled_if(move |data, _| data.contains(&heading))
          .empty_if_not(move |data, _| !data.contains(&heading)),
      );
    }

    column
      .lens(super::App::settings.then(Settings::headings))
      .on_click(|ctx, _, _| RootStack::dismiss(ctx))
      .boxed()
  }

  fn theme_picker_heading<T: Data + AsRef<str>>(
    collapsed: bool,
    padding: impl Into<Insets>,
  ) -> impl Widget<T> {
    let mut row = Flex::row()
      .with_child(lensed_bold(
        druid::theme::TEXT_SIZE_NORMAL,
        druid::FontWeight::SEMI_BOLD,
        druid::theme::TEXT_COLOR,
      ))
      .with_flex_spacer(1.0);

    if collapsed {
      row.add_child(Icon::new(*CHEVRON_LEFT))
    } else {
      row.add_child(Rotated::new(Icon::new(*CHEVRON_RIGHT), 1))
    }

    row.padding(padding.into())
  }

  fn theme_picker_expanded(themes: impl Iterator<Item = Themes>) -> impl Widget<super::App> {
    Flex::column()
      .with_child(Self::theme_picker_heading(false, (7.0, 7.0, 7.0, 0.0)))
      .tap(|col| {
        for theme in themes {
          col.add_child(
            Flex::column()
              .with_default_spacer()
              .with_child(
                CardButton::button_text(theme.as_ref())
                  .padding(7.0)
                  .expand_width()
                  .scope_with_hover_state(false, true, |widget| {
                    const THEME_OPTION_BORDER: Key<druid::Color> =
                      Key::new("settings.themes.option.border");

                    widget
                      .border(THEME_OPTION_BORDER, 1.0)
                      .env_scope(|env, data| {
                        env.set(
                          THEME_OPTION_BORDER,
                          if data.1 {
                            env.get(druid::theme::BORDER_LIGHT)
                          } else {
                            druid::Color::TRANSPARENT
                          },
                        )
                      })
                  })
                  .on_click(move |_, data, _| {
                    *data = theme;
                  }),
              )
              .empty_if_not(move |data, _| data != &theme),
          )
        }
      })
      .lens(App::settings.then(Settings::theme))
      .on_change(|_, _, data, _| {
        let _ = data.settings.save();
      })
      .on_click(|ctx, _, _| RootStack::dismiss(ctx))
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
      .map_err(Into::into)
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

  pub fn save_on_change(
    _ctx: &mut druid::EventCtx,
    _old: &Self,
    data: &mut Self,
    _env: &druid::Env,
  ) {
    if let Err(e) = data.save() {
      eprintln!("{:?}", e)
    }
  }

  fn save_on_command<P>(_ctx: &mut druid::EventCtx, _: &P, data: &mut Self) {
    if let Err(e) = data.save() {
      eprintln!("{:?}", e)
    }
  }

  pub fn save_async(&self, handle: &tokio::runtime::Handle) {
    let copy = self.clone();
    handle.spawn_blocking(move || copy.save());
  }
}

pub enum SettingsCommand {
  UpdateInstallDir(PathBuf),
  SelectInstallDir,
}

pub struct InstallDirDelegate;

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

use std::path::PathBuf;

use druid::{
  im::Vector,
  lens,
  text::ParseFormatter,
  widget::{
    Axis, Button, Checkbox, Flex, Label, Painter, TextBox, TextBoxEvent, ValidationDelegate,
    ViewSwitcher, WidgetExt,
  },
  Data, Lens, LensExt, Selector, Widget,
};
use druid_widget_nursery::{material_icons::Icon, wrap::Wrap, WidgetExt as _};
use extend::ext;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use tap::Tap;

use super::{
  controllers::HoverController,
  mod_list::headings::{Header, Heading},
  tools::vmparams::VMParams,
  util::{
    button_painter, default_true, h2_fixed, hoverable_text, icons::*, make_column_pair,
    make_flex_pair, CommandExt, LabelExt, LoadError, SaveError, ShadeColor, WidgetExtEx,
    WithHoverState,
  },
};
use crate::{
  app::PROJECT,
  theme::Themes,
  widgets::{card::Card, root_stack::RootStack},
};

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
  pub experimental_launch: bool,
  pub experimental_resolution: (u32, u32),
  #[serde(default = "default_true")]
  pub hide_webview_on_conflict: bool,
  #[serde(default = "default_true")]
  pub open_forum_link_in_webview: bool,
  #[serde(default = "default_headers")]
  #[data(eq)]
  pub headings: Vector<Heading>,
  pub show_auto_update_for_discrepancy: bool,
  pub theme: Themes,
  #[serde(skip)]
  pub vmparams: Option<VMParams>,
  pub vmparams_linked: bool,

  #[serde(default = "default_true")]
  pub show_duplicate_warnings: bool,
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
              Button::new("Browse...")
                .controller(HoverController::default())
                .on_click(|ctx, _, _| {
                  ctx.submit_command_global(Selector::new("druid.builtin.textbox-cancel-editing"));
                  ctx.submit_command_global(
                    Settings::SELECTOR.with(SettingsCommand::SelectInstallDir),
                  )
                }),
            ),
        )
        .with_default_spacer()
        .with_child(h2_fixed("Warn when overwriting '.git' folders:"))
        .with_child(Checkbox::from_label(Label::wrapped("Aimed at developers. If a mod folder is an active Git project this option will warn you if it would be overwritten or deleted")).lens(Settings::git_warn))
        .with_default_spacer()
        .with_child(h2_fixed("Use in-app browser to open links:"))
        .with_child(Checkbox::from_label(Label::wrapped("Uses an embedded browser when enabled. If disabled links will open in your system default web browser.")).lens(Settings::open_forum_link_in_webview))
        .with_default_spacer()
        .with_child(h2_fixed("Show automatic updates even for mods that have a version discrepancy"))
        .with_child(Checkbox::from_label(Label::wrapped("Indicates a mod has an update even when the installed version is a higher/more recent version than is available on the server. (Recommended Off)")).lens(Settings::show_auto_update_for_discrepancy))
        .with_default_spacer()
        .with_child(h2_fixed("Show a warning when more than one copy of a mod is installed:"))
        .with_child(Checkbox::from_label(Label::wrapped("When more than one copy of a mod is installed at the same time it is completely random which version is actually loaded by the game.")).lens(Settings::show_duplicate_warnings))
        .with_default_spacer()
        .with_child(h2_fixed("Edit columns:"))
        .with_child(Self::headings_editor())
        .with_default_spacer()
        .with_child(hoverable_text(Some(druid::Color::BLACK)).constant("Open config.json".to_owned()).on_click(|_, _, _| {
          let _ = opener::open(Settings::path(false));
        }).align_right().expand_width())
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
    const DISMISS_ADD_COLUMN_DROPDOWN: Selector =
      Selector::new("settings.add_column_dropdown.dismiss");

    ViewSwitcher::new(
      |headings: &Vector<Heading>, _| headings.clone(),
      |_, headings, _| {
        Wrap::new()
          .direction(Axis::Horizontal)
          .alignment(druid_widget_nursery::wrap::WrapAlignment::Start)
          .tap_mut(|row| {
            let mut height_linker = None;
            for (idx, heading) in headings.iter().cloned().enumerate() {
              row.add_child(
                Card::builder()
                  .with_shadow_length(6.)
                  .with_background(druid::theme::BACKGROUND_DARK)
                  .build(
                    Flex::row()
                      .with_default_spacer()
                      .with_child({
                        let icon = Icon::new(*ARROW_LEFT).background(button_painter());

                        if idx > 0 {
                          icon
                            .controller(HoverController::default())
                            .on_click(move |ctx, data: &mut Vector<Heading>, _| {
                              data.swap(idx - 1, idx);
                              ctx.submit_command(Header::SWAP_HEADINGS.with((idx - 1, idx)))
                            })
                            .boxed()
                        } else {
                          icon.disabled().invisible().boxed()
                        }
                      })
                      .with_flex_child(
                        Label::wrapped(format!("{}. {}", idx + 1, heading))
                          .with_text_alignment(druid::TextAlignment::Center)
                          .expand_width()
                          .padding((5.0, 0.0)),
                        1.,
                      )
                      .with_child(
                        Icon::new(*CLOSE)
                          .on_click(move |ctx, data: &mut Vector<Heading>, _| {
                            if data.len() > 1 {
                              data.retain(|existing| existing != &heading);
                              ctx.submit_command_global(Header::REMOVE_HEADING.with(heading));
                            }
                          })
                          .disabled_if({
                            let disabled = headings.len() <= 1;
                            move |_, _| disabled
                          })
                          .controller(HoverController::default()),
                      )
                      .with_child({
                        let icon = Icon::new(*ARROW_RIGHT).background(button_painter());

                        if idx < headings.len() - 1 {
                          icon
                            .controller(HoverController::default())
                            .on_click(move |ctx, data: &mut Vector<Heading>, _| {
                              data.swap(idx, idx + 1);
                              ctx.submit_command(Header::SWAP_HEADINGS.with((idx, idx + 1)))
                            })
                            .boxed()
                        } else {
                          icon.disabled().invisible().boxed()
                        }
                      })
                      .with_default_spacer()
                      .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
                      .must_fill_main_axis(true)
                      .padding((0., 5., 0., 5.)),
                  )
                  .padding(2.)
                  .fix_width(300.)
                  .link_height_with(&mut height_linker)
                  .boxed(),
              )
            }
            let missing = Heading::iter()
              .filter(|h| !headings.contains(h) && !matches!(h, Heading::Enabled | Heading::Score))
              .collect::<Vec<_>>();
            if !missing.is_empty() {
              row.add_child(
                Card::builder()
                  .with_insets((-4., 18.))
                  .with_shadow_length(6.0)
                  .with_shadow_increase(2.0)
                  .with_background(druid::Color::GRAY.lighter_by(9))
                  .hoverable_distinct(
                    {
                      move || {
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
                      }
                    },
                    {
                      move || {
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
                          .with_child(Icon::new(*ADD_CIRCLE))
                          .with_child(Icon::new(*ARROW_RIGHT).disabled().invisible())
                          .with_default_spacer()
                          .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
                          .must_fill_main_axis(true)
                          .padding((0., 5., 0., 5.))
                      }
                    },
                  )
                  .fix_width(300.)
                  .link_height_with(&mut height_linker)
                  .on_click(move |ctx, data, _| {
                    *data = true;
                    RootStack::show(
                      ctx,
                      ctx.window_origin(),
                      Self::add_column_dropdown(missing.clone()),
                      Some(|ctx: &mut druid::EventCtx| {
                        ctx.submit_command(DISMISS_ADD_COLUMN_DROPDOWN)
                      }),
                    )
                  })
                  .disabled_if(|data, _| *data)
                  .invisible_if(|data| *data)
                  .on_command(DISMISS_ADD_COLUMN_DROPDOWN, |_, _, data| *data = false)
                  .scope_independent(|| false)
                  .boxed(),
              )
            }
          })
          .boxed()
      },
    )
    .lens(Settings::headings)
  }

  fn add_column_dropdown(missing: Vec<Heading>) -> impl Fn() -> Box<dyn Widget<super::App>> {
    move || {
      let missing = missing.clone();

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
            .lens(lens!((T, bool), 0))
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

      Card::builder()
        .with_insets((-4., 18.))
        .with_shadow_length(6.0)
        .with_shadow_increase(2.0)
        .with_background(druid::Color::WHITE.darker())
        .hoverable(move || {
          let mut column = Flex::column()
            .with_child(
              Card::builder()
                .with_insets(0.0)
                .with_shadow_length(0.0)
                .with_background(druid::Color::GRAY.lighter_by(9))
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

          for heading in missing.clone() {
            column.add_child(
              Flex::padded_row()
                .with_content(
                  Label::wrapped(heading.to_string())
                    .with_text_alignment(druid::TextAlignment::Center),
                )
                .on_click(move |ctx, data: &mut Vector<Heading>, _| {
                  data.push_back(heading);
                  ctx.submit_command(Header::ADD_HEADING.with(heading))
                }),
            );
          }

          column
        })
        .fix_width(300.)
        .lens(super::App::settings.then(Settings::headings))
        .on_click(|ctx, _, _| RootStack::dismiss(ctx))
        .boxed()
    }
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
              .controller(HoverController::default())
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
        h2_fixed("Starsector Install Directory:"),
        Flex::for_axis(axis)
          .with_child(input.expand_width())
          .with_child(
            Button::new("Browse...")
              .controller(HoverController::default())
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

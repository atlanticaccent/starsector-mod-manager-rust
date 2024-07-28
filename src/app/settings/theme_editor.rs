use druid::{
  lens,
  lens::Constant,
  text::{FontDescriptor, FontWeight},
  widget::{Checkbox, Flex, Maybe, SizedBox, TextBox, ZStack},
  Color, Key, KeyOrValue, Lens, LensExt, Selector, UnitPoint, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as _};
use fake::Fake;
use strum::IntoEnumIterator;

use super::CHEVRON_LEFT;
use crate::{
  app::{
    util::{bolded, h3_fixed, lensed_bold, Tap, WidgetExtEx as _},
    SHUFFLE,
  },
  formatter::ParseOrLastFormatter,
  nav_bar::{Nav, NavLabel},
  patch::table::{FixedFlexTable, TableCellVerticalAlignment, TableColumnWidth, TableRow},
  theme::{ExtColor, Theme, Themes, OLD_BUTTON_DARK, OLD_BUTTON_LIGHT, OLD_TEXT_COLOR},
  widgets::{card::Card, card_button::CardButton, root_stack::RootStack},
};

const TEXT_BOX_FONT: Key<FontDescriptor> = Key::new("theme_editor.text_box.font");

pub struct ThemeEditor;

impl ThemeEditor {
  pub fn view() -> impl Widget<Theme> {
    let title_text = || {
      lensed_bold(
        druid::theme::TEXT_SIZE_NORMAL,
        druid::FontWeight::SEMI_BOLD,
        druid::theme::TEXT_COLOR,
      )
      .padding((8.0, 0.0))
    };

    Flex::column()
      .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
      .with_child(
        Flex::row()
          .with_child(
            Card::hoverable(
              || {
                ZStack::new(
                  title_text()
                    .lens(Constant("Back".to_owned()))
                    .padding((8.0, 0.0, 0.0, 0.0)),
                )
                .with_aligned_child(Icon::new(*CHEVRON_LEFT), UnitPoint::LEFT)
                .valign_centre()
                .expand_height()
              },
              (0.0, 14.0),
            )
            .fix_height(52.0)
            .padding((0.0, 5.0))
            .on_click(|ctx, _, _| ctx.submit_command(Nav::NAV_SELECTOR.with(NavLabel::Settings))),
          )
          .with_flex_child(
            Card::builder()
              .with_insets((0.0, 14.0))
              .with_corner_radius(4.0)
              .with_shadow_length(6.0)
              .build(
                title_text()
                  .lens(Constant("Settings  /  Theme Editor".to_owned()))
                  .valign_centre()
                  .align_left(),
              )
              .fix_height(52.0)
              .padding((0.0, 5.0))
              .expand_width(),
            1.0,
          )
          .expand_width(),
      )
      .with_flex_child(
        Flex::row()
          .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .with_child(
            Card::builder()
              .build(
                Flex::column()
                  .must_fill_main_axis(true)
                  .with_child(
                    FixedFlexTable::new()
                      .default_vertical_alignment(TableCellVerticalAlignment::Middle)
                      .default_column_width(TableColumnWidth::Intrinsic)
                      .row_border(Color::TRANSPARENT, 4.0)
                      .column_border(Color::TRANSPARENT, 10.0)
                      .with_row(optional_row("Text", Theme::text, OLD_TEXT_COLOR))
                      .with_row(optional_row(
                        "Button Dark",
                        Theme::button_dark,
                        OLD_BUTTON_DARK,
                      ))
                      .with_row(optional_row(
                        "Button Light",
                        Theme::button_light,
                        OLD_BUTTON_LIGHT,
                      ))
                      .with_row(required_row("Background Dark", Theme::background_dark))
                      .with_row(required_row("Background Light", Theme::background_light))
                      .with_row(required_row("Border Dark", Theme::border_dark))
                      .with_row(required_row("Border Light", Theme::border_light))
                      .with_row(optional_row("Shadow", Theme::shadow, Color::BLACK))
                      .with_row(optional_row(
                        "Action Background",
                        Theme::action,
                        Color::from(Theme::ACTION),
                      ))
                      .with_row(optional_row(
                        "Action Text",
                        Theme::action_text,
                        Color::from(Theme::ACTION_TEXT),
                      ))
                      .with_row(optional_row(
                        "Success Background",
                        Theme::success,
                        Color::from(Theme::SUCCESS),
                      ))
                      .with_row(optional_row(
                        "Success Text",
                        Theme::success_text,
                        Color::from(Theme::SUCCESS_TEXT),
                      ))
                      .with_row(optional_row(
                        "Error Background",
                        Theme::error,
                        Color::from(Theme::ERROR),
                      ))
                      .with_row(optional_row(
                        "Error Text",
                        Theme::error_text,
                        Color::from(Theme::ERROR_TEXT),
                      ))
                      .with_row(optional_row(
                        "Warning Background",
                        Theme::warning,
                        Color::from(Theme::WARNING),
                      ))
                      .with_row(optional_row(
                        "Warning Text",
                        Theme::warning_text,
                        Color::from(Theme::WARNING_TEXT),
                      ))
                      .with_row(optional_row(
                        "Do Not Ignore Background",
                        Theme::do_not_ignore,
                        Color::from(Theme::DO_NOT_IGNORE),
                      ))
                      .with_row(optional_row(
                        "Do Not Ignore Text",
                        Theme::do_not_ignore_text,
                        Color::from(Theme::DO_NOT_IGNORE_TEXT),
                      )),
                  )
                  .env_scope(|env, _| {
                    let font = env
                      .get(druid::theme::UI_FONT)
                      .with_size(18.0)
                      .with_weight(FontWeight::MEDIUM);
                    env.set(TEXT_BOX_FONT, font)
                  })
                  .padding((20.0, 5.0))
                  .scroll()
                  .vertical(),
              )
              .expand_height(),
          )
          .with_flex_child(
            Flex::column()
              .with_child(
                Card::builder()
                  .with_shadow_length(8.0)
                  .with_shadow_increase(0.0)
                  .with_border(1.0, druid::theme::BORDER_LIGHT)
                  .hoverable(|_| CardButton::button_text("Randomise All").padding((7.0, 5.0)))
                  .on_click(|_, theme, _| *theme = Theme::random())
                  .fix_width(150.0),
              )
              .with_child(
                Card::builder()
                  .with_shadow_length(8.0)
                  .with_shadow_increase(0.0)
                  .with_border(1.0, druid::theme::BORDER_LIGHT)
                  .stacked_button(
                    |_| {
                      super::Settings::theme_picker_heading(true, (7.0, 3.0)).constant("Load from:")
                    },
                    |_| theme_picker_expanded(Themes::iter().filter(|t| t != &Themes::Custom)),
                    CardButton::stack_none(),
                    150.0,
                  ),
              ),
            1.0,
          ),
        1.0,
      )
      .must_fill_main_axis(true)
      .expand()
      .on_command(RESET_CUSTOM_TO, |_, theme, custom| {
        *custom = (*theme).into();
      })
  }
}

const COLOR_KEY: Key<Color> = Key::new("theme_editor.preview.key");

fn required_row(
  label: &str,
  lens: impl Lens<Theme, ExtColor> + Clone + 'static,
) -> TableRow<Theme> {
  TableRow::new()
    .with_child(SizedBox::empty())
    .with_child(h3_fixed(label))
    .with_child(
      TextBox::new()
        .with_font(TEXT_BOX_FONT)
        .with_formatter(ParseOrLastFormatter::new())
        .update_data_while_editing(true)
        .lens(lens.clone()),
    )
    .with_child(
      SizedBox::empty()
        .fix_size(40.0, 40.0)
        .background(COLOR_KEY)
        .rounded(20.0)
        .border(Color::BLACK, 2.0)
        .env_scope(move |env, data: &ExtColor| env.set(COLOR_KEY, data.clone()))
        .lens(lens.clone()),
    )
    .with_child(randomise_colour(
      lens.map(|val| Some(val.clone()), |val, inner| *val = inner.unwrap()),
    ))
}

fn optional_row(
  label: &str,
  lens: impl Lens<Theme, Option<ExtColor>> + Clone + 'static,
  default: impl Into<KeyOrValue<Color>>,
) -> TableRow<Theme> {
  let default = default.into();

  TableRow::new()
    .with_child(
      Checkbox::new("")
        .stack_tooltip_custom(
          Card::new(
            bolded("When disabled this component's colour will use the indicated default instead.")
              .padding((7.0, 0.0)),
          )
          .fix_width(225.0),
        )
        .with_offset((10.0, 10.0))
        .on_click(|_, data, _| *data = !*data)
        .lens(
          druid::lens::Map::new(
            |(val, _): &(Option<ExtColor>, bool)| (val.clone(), val.is_some()),
            |state, inner| *state = inner,
          )
          .then(lens!((Option<ExtColor>, bool), 1)),
        )
        .on_change({
          let default = default.clone();
          move |_, _, (val, inner), env| {
            if !*inner {
              val.take();
            } else if val.is_none() {
              val.replace(default.resolve(env).into());
            }
          }
        })
        .scope(
          |data| (data.clone(), data.is_some()),
          lens!((Option<ExtColor>, bool), 0),
        )
        .lens(lens.clone())
        .wrap_with_hover_state(true, true),
    )
    .with_child(h3_fixed(label))
    .with_child(
      Maybe::new(
        || {
          TextBox::new()
            .with_font(TEXT_BOX_FONT)
            .with_formatter(ParseOrLastFormatter::new())
            .update_data_while_editing(true)
        },
        {
          let default = default.clone();
          move || {
            let default = default.clone();
            TextBox::new()
              .with_font(TEXT_BOX_FONT)
              .with_placeholder("#ffffff")
              .on_added(move |text_box, _, _, env| {
                let color: ExtColor = default.resolve(env).into();
                text_box.set_placeholder(color.to_string())
              })
              .constant(String::new())
              .disabled()
          }
        },
      )
      .lens(lens.clone()),
    )
    .with_child(
      SizedBox::empty()
        .fix_size(40.0, 40.0)
        .background(COLOR_KEY)
        .rounded(20.0)
        .border(Color::BLACK, 2.0)
        .env_scope(move |env, data: &Option<ExtColor>| {
          env.set(
            COLOR_KEY,
            data.clone().unwrap_or(default.resolve(env).into()),
          )
        })
        .lens(lens.clone()),
    )
    .with_child(randomise_colour(lens))
}

fn randomise_colour(lens: impl Lens<Theme, Option<ExtColor>>) -> impl Widget<Theme> {
  Icon::new(*SHUFFLE)
    .stack_tooltip_custom(
      Card::new(bolded("Randomise the colour of this element.").padding((7.0, 0.0)))
        .fix_width(225.0),
    )
    .with_offset((10.0, 10.0))
    .on_click(|_, data: &mut Option<ExtColor>, _| {
      let color: String = fake::faker::color::en::HexColor().fake();
      data.replace(Color::from_hex_str(&color).unwrap().into());
    })
    .wrap_with_hover_state(true, true)
    .lens(lens)
}

const RESET_CUSTOM_TO: Selector<Themes> = Selector::new("theme_editor.set_to_theme");

fn theme_picker_expanded(themes: impl Iterator<Item = Themes>) -> impl Widget<super::App> {
  Flex::column()
    .with_child(
      super::Settings::theme_picker_heading(false, (7.0, 3.0, 7.0, 0.0)).constant("Load from:"),
    )
    .tap(|col| {
      for theme in themes {
        col.add_child(
          Flex::column().with_default_spacer().with_child(
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
              .on_click(move |ctx, data, _| {
                *data = theme;
                ctx.submit_command(RESET_CUSTOM_TO.with(theme))
              }),
          ),
        )
      }
    })
    .on_click(|ctx, _, _| RootStack::dismiss(ctx))
    .scope_independent(|| Themes::default())
}

use druid::{theme, widget::Flex, Data, Widget, WidgetExt as _, lens};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as _};

use crate::{
  app::{
    controllers::{HeightLinker, HeightLinkerShared},
    mod_list::install::install_options::InstallOptions,
    util::{bold_text, WidgetExtEx as _, WithHoverState, ADD_BOX, CHECKBOX},
  },
  widgets::card::Card,
};

use super::filter_button::FilterButton;

pub struct FilterOptions;

impl FilterOptions {
  pub fn view() -> impl Widget<bool> {
    Card::builder()
      .with_insets((0.0, 14.0))
      .with_corner_radius(4.0)
      .with_shadow_length(8.0)
      .build(
        FilterButton::inner()
          .fix_height(60.0)
          .padding((-8.0, 0.0, -8.0, -4.0)),
      )
      .or_empty(|data: &bool, _| *data)
      .fix_width(super::FILTER_WIDTH)
      .on_command(InstallOptions::DISMISS, |ctx, payload, data| {
        let hitbox = ctx
          .size()
          .to_rect()
          .with_origin(ctx.to_window((0.0, 0.0).into()));
        *data = hitbox.contains(*payload);
      })
  }

  pub fn wide_view() -> impl Widget<bool> {
    let mut width_linker = {
      let mut linker = HeightLinker::new();
      linker.axis = druid::widget::Axis::Horizontal;
      Some(linker.into_shared())
    };
    Card::builder()
      .with_insets((0.0, 14.0))
      .with_corner_radius(4.0)
      .with_shadow_length(8.0)
      .with_background(theme::BACKGROUND_DARK)
      .build(FilterButton::button_styling(
        Flex::row()
          .with_child(
            Card::builder().with_insets((0.0, 10.0)).build(
              Flex::column()
                .with_child(Self::option("Enabled", &mut width_linker))
                .with_child(Self::option("Disabled", &mut width_linker))
                .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start),
            ),
          )
          .with_child(
            Card::builder().with_insets((0.0, 10.0)).build(
              Flex::column()
                .with_child(Self::option("Enabled", &mut width_linker))
                .with_child(Self::option("Disabled", &mut width_linker))
                .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start),
            ),
          )
          .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
          .expand_width(),
      ))
      .or_empty(|data: &bool, _| *data)
      .on_command(InstallOptions::DISMISS, |ctx, payload, data| {
        let hitbox = ctx
          .size()
          .to_rect()
          .with_origin(ctx.to_window((0.0, 0.0).into()));
        *data = hitbox.contains(*payload);
      })
  }

  fn option_text<T: Data>(text: &str) -> impl Widget<T> {
    bold_text(
      text,
      druid::theme::TEXT_SIZE_NORMAL,
      druid::FontWeight::SEMI_BOLD,
      druid::theme::TEXT_COLOR,
    )
    .padding((8.0, 0.0))
  }

  fn option<T: Data>(text: &str, width_linker: &mut Option<HeightLinkerShared>) -> impl Widget<T> {
    Flex::row()
      .with_child(
        Icon::new(CHECKBOX)
          .else_if(|data, _| *data, Icon::new(ADD_BOX))
          .padding((5.0, 0.0, -5.0, 0.0)),
      )
      .with_child(Self::option_text(text))
      .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
      .lens(lens!((bool, bool), 0))
      .with_hover_state(false)
      .on_click(|_ctx, data, _| *data = !*data)
      .scope_independent(|| false)
      .link_height_with(width_linker)
  }
}

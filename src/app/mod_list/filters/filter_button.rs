use druid::{widget::Flex, Data, Widget, WidgetExt as _};
use druid_widget_nursery::material_icons::Icon;

use crate::{
  app::util::{bold_text, WidgetExtEx as _, TUNE},
  widgets::card::Card,
};

use super::FILTER_POSITION;

pub struct FilterButton;

impl FilterButton {
  pub fn inner<T: Data>() -> Flex<T> {
    Flex::column().with_child(
      Flex::row()
        .with_child(bold_text(
          "Filters",
          druid::theme::TEXT_SIZE_NORMAL,
          druid::FontWeight::SEMI_BOLD,
          druid::theme::TEXT_COLOR,
        ))
        .with_child(Icon::new(TUNE)),
    )
  }

  pub fn button_styling<T: Data>(inner: impl Widget<T> + 'static) -> impl Widget<T> {
    inner.padding((8.0, 0.0))
  }

  fn button<T: Data>() -> impl Widget<T> {
    Self::button_styling(Self::inner())
  }

  pub fn view() -> impl Widget<bool> {
    Card::builder()
      .with_insets((0.0, 14.0))
      .hoverable(|| FilterButton::button())
      .on_click(|ctx, data, _| {
        ctx.submit_command(FILTER_POSITION.with(ctx.window_origin()));
        *data = true;
      })
      .on_added(|_, ctx, _, _| ctx.submit_command(FILTER_POSITION.with(ctx.window_origin())))
      .or_empty(|data, _| !*data)
      .fix_size(super::FILTER_WIDTH, 52.0)
  }
}

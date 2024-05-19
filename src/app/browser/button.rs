use druid::{Data, Widget, WidgetExt as _};

use crate::{
  app::util::{bold_text, WidgetExtEx as _},
  widgets::card::Card,
};

pub fn button_text<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(
    text,
    druid::theme::TEXT_SIZE_NORMAL,
    druid::FontWeight::SEMI_BOLD,
    druid::theme::TEXT_COLOR,
  )
}

pub fn button_styling<T: Data>(inner: impl Widget<T> + 'static) -> impl Widget<T> {
  inner.padding((8.0, 0.0))
}

pub fn button<T: Data, W: Widget<T> + 'static>(inner: impl Fn(bool) -> W) -> impl Widget<T> {
  Card::builder()
    .with_insets((0.0, 14.0))
    .hoverable_distinct(
      || button_styling(inner(false).lens(druid::lens!((T, bool), 0))),
      || button_styling(inner(true).lens(druid::lens!((T, bool), 0))),
    )
    .on_click(|_, data, _| data.1 = true)
    .scope(|data| (data, false), druid::lens!((T, bool), 0))
    .fix_size(175., 52.)
}

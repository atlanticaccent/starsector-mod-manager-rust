use std::rc::Rc;

use druid::{Data, Selector, Widget, WidgetExt as _, WidgetId};
use druid_widget_nursery::WidgetExt;

use super::root_stack::RootStack;
use crate::{
  app::util::{bold_text, WidgetExtEx as _},
  widgets::card::Card,
};

pub struct CardButton;

impl CardButton {
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

  pub fn button_unconstrained<T: Data, W: Widget<T> + 'static>(
    inner: impl Fn(bool) -> W,
  ) -> impl Widget<T> {
    Card::builder()
      .with_insets((0.0, 14.0))
      .hoverable_distinct(
        || Self::button_styling(inner(false).lens(druid::lens!((T, bool), 0))),
        || Self::button_styling(inner(true).lens(druid::lens!((T, bool), 0))),
      )
      .on_click(|_, data, _| data.1 = true)
      .scope(|data| (data, false), druid::lens!((T, bool), 0))
  }

  pub fn button<T: Data, W: Widget<T> + 'static>(
    inner: impl Fn(bool) -> W + 'static,
  ) -> impl Widget<T> {
    Self::button_unconstrained(inner).fix_height(52.)
  }

  fn dropdown_maker<W: Widget<crate::app::App> + 'static>(
    inner: impl Fn(bool) -> W + 'static,
    width: f64,
  ) -> impl Fn() -> Box<dyn Widget<crate::app::App>> {
    let inner = Rc::new(inner);
    move || {
      let inner = inner.clone();
      Self::button_unconstrained(move |hover| (inner.clone())(hover))
        .fix_width(width)
        .boxed()
    }
  }

  pub fn stacked_dropdown<
    T: Data,
    W: Widget<T> + 'static,
    WO: Widget<crate::app::App> + 'static,
  >(
    base: impl Fn(bool) -> W + 'static,
    dropdown: impl Fn(bool) -> WO + 'static,
    width: f64,
  ) -> impl Widget<T> {
    const DROPDOWN_DISMISSED: Selector = Selector::new("stacked_dropdown.re-enable");

    let id = WidgetId::next();
    let dropdown = Rc::new(Self::dropdown_maker(dropdown, width));

    Self::button(base)
      .fix_width(width)
      .scope_with(false, move |widget| {
        widget
          .on_click(move |ctx, data, _| {
            data.inner = true;
            let dropdown = dropdown.clone();
            RootStack::show(
              ctx,
              ctx.window_origin(),
              move || (dropdown)(),
              Some(move |ctx: &mut druid::EventCtx| ctx.submit_command(DROPDOWN_DISMISSED.to(id))),
            )
          })
          .invisible_if(|data| data.inner)
          .disabled_if(|data, _| data.inner)
          .on_command(DROPDOWN_DISMISSED, |_, _, data| data.inner = false)
          .with_id(id)
      })
  }
}

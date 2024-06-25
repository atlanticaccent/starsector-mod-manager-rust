use std::rc::Rc;

use druid::{Data, Selector, Widget, WidgetExt as _, WidgetId};
use druid_widget_nursery::{CommandCtx, LaidOutCtx, WidgetExt};

use super::{card::CardBuilder, root_stack::RootStack};
use crate::{
  app::{
    util::{bold_text, State, WidgetExtEx as _},
    App,
  },
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

  pub fn button_unconstrained<T: Data, W: Widget<T> + 'static>(
    inner: impl Fn(bool) -> W,
  ) -> impl Widget<T> {
    Self::button_unconstrained_with(inner, Card::builder())
  }

  pub fn button_unconstrained_with<T: Data, W: Widget<T> + 'static>(
    inner: impl Fn(bool) -> W,
    builder: CardBuilder,
  ) -> impl Widget<T> {
    builder
      .with_insets((0.0, 14.0))
      .hoverable_distinct(
        || inner(false).lens(druid::lens!((T, bool), 0)),
        || inner(true).lens(druid::lens!((T, bool), 0)),
      )
      .on_click(|_, data, _| data.1 = true)
      .scope(|data| (data, false), druid::lens!((T, bool), 0))
  }

  pub fn button<T: Data, W: Widget<T> + 'static>(
    inner: impl Fn(bool) -> W + 'static,
  ) -> impl Widget<T> {
    Self::button_with(inner, Card::builder()).fix_height(52.)
  }

  pub fn button_with<T: Data, W: Widget<T> + 'static>(
    inner: impl Fn(bool) -> W + 'static,
    builder: CardBuilder,
  ) -> impl Widget<T> {
    Self::button_unconstrained_with(inner, builder)
  }
  fn dropdown_maker<T: Data, W: Widget<T> + 'static>(
    inner: impl Fn(bool) -> W + 'static,
    width: f64,
  ) -> impl Fn() -> Box<dyn Widget<T>> {
    Self::dropdown_maker_with(inner, width, Card::builder())
  }

  fn dropdown_maker_with<T: Data, W: Widget<T> + 'static>(
    inner: impl Fn(bool) -> W + 'static,
    width: f64,
    builder: CardBuilder,
  ) -> impl Fn() -> Box<dyn Widget<T>> {
    let inner = Rc::new(inner);
    move || {
      let inner = inner.clone();
      Self::button_unconstrained_with(move |hover| inner(hover).expand_width(), builder.clone())
        .fix_width(width)
        .boxed()
    }
  }

  pub fn stacked_dropdown<T: Data, W: Widget<T> + 'static, WO: Widget<App> + 'static>(
    base: impl Fn(bool) -> W + 'static,
    dropdown: impl Fn(bool) -> WO + 'static,
    width: f64,
  ) -> impl Widget<T> {
    #[allow(unused_assignments)]
    let mut type_nonesense = Some(|widget, _, _| widget);
    type_nonesense = None;

    Self::stacked_dropdown_with_options(base, dropdown, type_nonesense, width, Card::builder())
  }

  pub fn stacked_dropdown_with_options<
    T: Data,
    W: Widget<T> + 'static,
    WO: Widget<App> + 'static,
    WSO: Widget<State<T, bool>> + 'static,
  >(
    base: impl Fn(bool) -> W + 'static,
    dropdown: impl Fn(bool) -> WO + 'static,
    alt_stack_activation: Option<
      impl Fn(
          ScopedStackCardButton<T>,
          Rc<dyn Fn() -> Box<dyn Widget<App>> + 'static>,
          WidgetId,
        ) -> WSO
        + 'static,
    >,
    width: f64,
    builder: CardBuilder,
  ) -> impl Widget<T> {
    let id = WidgetId::next();
    let dropdown: Rc<dyn Fn() -> Box<dyn Widget<App>>> =
      Rc::new(Self::dropdown_maker_with(dropdown, width, builder.clone()));

    Self::button_with(base, builder)
      .fix_width(width)
      .scope_with(false, move |widget| {
        if let Some(alt) = alt_stack_activation {
          alt(widget, dropdown, id).boxed()
        } else {
          widget
            .on_click(move |ctx, data, _| {
              Self::trigger_dropdown_manually(ctx, dropdown.clone(), id, data)
            })
            .boxed()
        }
        .invisible_if(|data| data.inner)
        .disabled_if(|data, _| data.inner)
        .on_command(DROPDOWN_DISMISSED, |_, _, data| data.inner = false)
        .with_id(id)
      })
  }

  pub fn trigger_dropdown_manually<T: Data>(
    ctx: &mut (impl CommandCtx + LaidOutCtx),
    dropdown: Rc<dyn Fn() -> Box<dyn Widget<App>> + 'static>,
    id: WidgetId,
    data: &mut State<T, bool>,
  ) {
    data.inner = true;
    RootStack::show(
      ctx,
      ctx.window_origin(),
      move || (dropdown)(),
      Some(move |ctx: &mut druid::EventCtx| ctx.submit_command(DROPDOWN_DISMISSED.to(id))),
    )
  }
}

pub type ScopedStackCardButton<T> = druid::widget::LensWrap<
  crate::app::util::State<T, bool>,
  T,
  crate::app::util::state_derived_lenses::outer<T, bool>,
  druid::widget::SizedBox<T>,
>;
const DROPDOWN_DISMISSED: Selector = Selector::new("stacked_dropdown.re-enable");

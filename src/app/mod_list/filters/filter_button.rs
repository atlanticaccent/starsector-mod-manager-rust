use druid::{
  widget::{Flex, SizedBox},
  Data, Widget, WidgetExt as _,
};
use druid_widget_nursery::{material_icons::Icon, Stack, StackChildPosition};

use crate::{
  app::{
    mod_list::ModList,
    util::{bold_text, WidgetExtEx as _, TUNE},
    CLEAR,
  },
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
        .with_child(Icon::new(*TUNE)),
    )
  }

  pub fn button_styling<T: Data>(inner: impl Widget<T> + 'static) -> impl Widget<T> {
    inner.padding((8.0, 0.0))
  }

  fn button<T: Data>() -> impl Widget<T> {
    Self::button_styling(Self::inner())
  }

  fn reset<T: Data>() -> impl Widget<T> {
    Self::button_styling(Icon::new(*CLEAR).padding((-4.0, -4.0))).padding((
      0.0,
      0.0,
      super::FILTER_WIDTH - 16.0,
      0.0,
    ))
  }

  pub fn view() -> impl Widget<(bool, bool)> {
    Stack::new()
      .with_child(
        Card::builder()
          .with_insets((0.0, 14.0))
          .with_shadow_length(4.0)
          .hoverable(|| FilterButton::reset())
          .fix_height(46.0)
          .on_click(|ctx, _, _| ctx.submit_command(ModList::FILTER_RESET))
          .else_if(
            |(_, data): &(bool, bool), _| !*data,
            SizedBox::empty()
              .width(super::FILTER_WIDTH - 6.0)
              .height(46.0),
          )
          .padding((0.0, 4.0, 6.0, 0.0)),
      )
      .with_positioned_child(
        Card::builder()
          .with_insets((0.0, 14.0))
          .hoverable(|| FilterButton::button())
          .on_click(|ctx, (data, _), _| {
            ctx.submit_command(FILTER_POSITION.with(ctx.window_origin()));
            *data = true;
          })
          .on_added(|_, ctx, _, _| ctx.submit_command(FILTER_POSITION.with(ctx.window_origin())))
          .or_empty(|(data, _), _| !*data)
          .fix_size(super::FILTER_WIDTH, 52.0),
        StackChildPosition::new().right(Some(0.0)),
      )
  }
}

use common::{labels::bold_text, widget_ext::WidgetExtEx, widgets::card::Card};
use druid::{
  widget::{Flex, SizedBox},
  Data, Widget, WidgetExt as _,
};
use druid_widget_nursery::material_icons::Icon;
use icons::HANDYMAN;

use super::ActionsState;

pub struct ActionsButton;

impl ActionsButton {
  pub fn inner<T: Data>(_: bool) -> Flex<T> {
    Flex::column().with_child(
      Flex::row()
        .with_child(bold_text(
          "Actions",
          druid::theme::TEXT_SIZE_NORMAL,
          druid::FontWeight::SEMI_BOLD,
          druid::theme::TEXT_COLOR,
        ))
        .with_spacer(4.0)
        .with_child(Icon::new(*HANDYMAN)),
    )
  }

  pub fn button_styling<T: Data>(inner: impl Widget<T> + 'static) -> impl Widget<T> {
    inner.padding((8.0, 0.0))
  }

  fn button<T: Data>(filled: bool) -> impl Widget<T> {
    Self::button_styling(Self::inner(filled))
  }

  pub fn view() -> impl Widget<ActionsState> {
    Card::builder()
      .with_insets((0.0, 14.0))
      .hoverable(|_| ActionsButton::button(false))
      .on_click(|_, data: &mut ActionsState, _| data.open = true)
      .else_if(|data, _| data.open, SizedBox::empty())
      .fix_size(super::INSTALL_WIDTH, 52.0)
  }
}

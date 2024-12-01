use common::{
  labels::bold_text,
  widget_ext::WidgetExtEx,
  widgets::{card::Card, rotate::Rotated},
};
use druid::{
  widget::{Flex, SizedBox},
  Data, Widget, WidgetExt as _,
};
use druid_widget_nursery::material_icons::Icon;
use icons::TOGGLE_ON;

use super::ToggleState;

pub struct ToggleButton;

impl ToggleButton {
  pub fn inner<T: Data>(filled: bool) -> Flex<T> {
    let mut row = Flex::row().with_child(bold_text(
      "Toggle Mods",
      druid::theme::TEXT_SIZE_NORMAL,
      druid::FontWeight::SEMI_BOLD,
      druid::theme::TEXT_COLOR,
    ));
    if filled {
      row.add_child(Rotated::new(Icon::new(*TOGGLE_ON), 2));
    } else {
      row.add_child(Icon::new(*TOGGLE_ON));
    }

    Flex::column().with_child(row)
  }

  pub fn button_styling<T: Data>(inner: impl Widget<T> + 'static) -> impl Widget<T> {
    inner.padding((8.0, 0.0))
  }

  fn button<T: Data>(filled: bool) -> impl Widget<T> {
    Self::button_styling(Self::inner(filled))
  }

  pub fn view() -> impl Widget<ToggleState> {
    Card::builder()
      .with_insets((0.0, 14.0))
      .hoverable(|_| ToggleButton::button(false))
      .on_click(|_, data: &mut ToggleState, _| data.open = true)
      .else_if(|data, _| data.open, SizedBox::empty())
      .fix_size(super::INSTALL_WIDTH, 52.0)
  }
}

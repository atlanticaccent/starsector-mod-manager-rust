use druid::{
  widget::{Flex, SizedBox},
  Data, Widget, WidgetExt as _,
};
use druid_widget_nursery::material_icons::Icon;

use super::InstallState;
use crate::{
  app::util::{bold_text, WidgetExtEx as _, ADD_CIRCLE, ADD_CIRCLE_OUTLINE},
  widgets::card::Card,
};

pub struct InstallButton;

impl InstallButton {
  pub fn inner<T: Data>(filled: bool) -> Flex<T> {
    Flex::column().with_child(
      Flex::row()
        .with_child(bold_text(
          "Install Mod(s)",
          druid::theme::TEXT_SIZE_NORMAL,
          druid::FontWeight::SEMI_BOLD,
          druid::theme::TEXT_COLOR,
        ))
        .with_child(Icon::new(if filled {
          ADD_CIRCLE
        } else {
          ADD_CIRCLE_OUTLINE
        })),
    )
  }

  pub fn button_styling<T: Data>(inner: impl Widget<T> + 'static) -> impl Widget<T> {
    inner.padding((8.0, 0.0))
  }

  fn button<T: Data>(filled: bool) -> impl Widget<T> {
    Self::button_styling(Self::inner(filled))
  }

  pub fn view() -> impl Widget<InstallState> {
    Card::builder()
      .with_insets((0.0, 14.0))
      .hoverable_distinct(
        || InstallButton::button(false),
        || InstallButton::button(true),
      )
      .on_click(|_, data: &mut InstallState, _| data.open = true)
      .else_if(|data, _| data.open, SizedBox::empty())
      .fix_size(super::INSTALL_WIDTH, 52.0)
  }
}

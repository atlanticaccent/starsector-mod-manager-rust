use druid::{
  widget::{Either, Flex, SizedBox},
  Data, UnitPoint, Widget, WidgetExt as _,
};
use druid_widget_nursery::material_icons::Icon;

use crate::app::{
  controllers::ExtensibleController,
  util::{bold_text, Card, WidgetExtEx as _, ADD_CIRCLE, ADD_CIRCLE_OUTLINE},
};

use crate::app::ModList;

use super::InstallState;

pub struct InstallButton;

impl InstallButton {
  fn button<T: Data>(shadow: f64, filled: bool) -> impl Widget<T> {
    Card::builder((0.0, 14.0))
      .with_corner_radius(4.0)
      .with_shadow_length(shadow)
      .build(
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
          }))
          .padding((8.0, 0.0))
          .align_vertical(UnitPoint::CENTER)
          .fix_height(20.),
      )
  }

  pub fn view() -> impl Widget<InstallState> {
    Either::new(
      |data: &InstallState, _| data.hovered,
      InstallButton::button(8.0, true),
      InstallButton::button(6.0, false)
    )
    .on_event(|_, ctx, event, data: &mut InstallState| {
      if let druid::Event::MouseMove(_) = event {
        ctx.set_cursor(&druid::Cursor::Pointer);
        data.hovered = true;
        ctx.request_paint();
      } else if let druid::Event::Command(cmd) = event && cmd.is(ModList::INSTALL_BUTTON_STATE_CHANGE) {
        data.hovered = false;
        ctx.clear_cursor()
      }
      ctx.request_paint();
      false
    })
    .controller(ExtensibleController::new().on_lifecycle(|_, ctx, event, _, _| {
      if let druid::LifeCycle::HotChanged(false) = event {
        ctx.submit_command(ModList::INSTALL_BUTTON_STATE_CHANGE)
      }
    }))
    .on_click(|_, data, _| data.open = true)
    .else_if(|data, _| data.open, SizedBox::empty().fix_height(48.0))
  }
}
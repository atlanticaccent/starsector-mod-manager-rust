use druid::{widget::Flex, Data, Widget, WidgetExt};
use druid_widget_nursery::material_icons::Icon;

use crate::{
  app::{util::bold_text, App, REFRESH},
  widgets::card::Card,
};

pub struct Refresh;

impl Refresh {
  pub fn view<T: Data>() -> impl Widget<T> {
    Card::builder()
      .with_insets((0.0, 14.0))
      .hoverable(|| {
        Flex::row()
          .with_child(bold_text(
            "Refresh",
            druid::theme::TEXT_SIZE_NORMAL,
            druid::FontWeight::SEMI_BOLD,
            druid::theme::TEXT_COLOR,
          ))
          .with_child(Icon::new(*REFRESH))
          .align_horizontal(druid::UnitPoint::CENTER)
          .fix_width(175.0)
      })
      .on_click(|ctx, _, _| ctx.submit_command(App::REFRESH))
  }
}

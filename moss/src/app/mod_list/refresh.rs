use common::widgets::card::Card;
use druid::{Data, Widget, WidgetExt};
use druid_widget_nursery::material_icons::Icon;

use crate::app::{App, REFRESH};

pub struct Refresh;

impl Refresh {
  pub fn view<T: Data>() -> impl Widget<T> {
    Card::builder()
      .with_insets((14.0, 14.0))
      .hoverable(|_| Icon::new(*REFRESH))
      .fix_size(52.0, 52.0)
      // .stack_tooltip_custom(Card::new(bolded("Refresh")))
      // .with_offset((8.0, 8.0))
      .on_click(|ctx, _, _| ctx.submit_command(App::REFRESH))
  }
}

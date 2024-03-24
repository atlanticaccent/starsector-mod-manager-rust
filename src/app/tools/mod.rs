use druid::{widget::{Flex, Maybe}, Data, Lens, Widget, WidgetExt};

use crate::widgets::card::{Card, CardBuilder};

use self::vmparams::VMParams;

pub mod jre;
pub mod vmparams;

#[derive(Debug, Clone, Data, Lens)]
pub struct Tools {
  pub vmparams: Option<VMParams>
}

impl Tools {
  pub fn view() -> impl Widget<Self> {
    Flex::column()
      .must_fill_main_axis(true)
      .with_child(Maybe::or_empty(|| VMParams::view()).lens(Tools::vmparams))
  }
}

pub fn tool_card<T: Data>() -> CardBuilder<T> {
  Card::builder().with_insets((0.0, 14.0))
    .with_corner_radius(4.0)
    .with_shadow_length(6.0)
}

use druid::{widget::SizedBox, Data, Widget};

use crate::app::util::{Card, WidgetExtEx};

use super::InstallState;

pub struct InstallOptions;

impl InstallOptions {
  pub fn view() -> impl Widget<InstallState> {
    Card::new(SizedBox::empty().width(100.0).height(100.0))
      .or_empty(|data: &InstallState, _| data.open)
  }
}

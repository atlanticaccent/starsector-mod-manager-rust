use druid::{
  im::Vector,
  widget::{Label, List},
  Widget,
};

use super::util::{LabelExt as _, WidgetExtEx};

pub struct Activity;

impl Activity {
  pub fn view() -> impl Widget<Vector<String>> {
    List::new(|| Label::wrapped_func(|val: &String, _| val.clone())).in_card()
  }
}

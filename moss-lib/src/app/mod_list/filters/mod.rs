use std::collections::HashSet;

use druid::{Data, Lens, Point, Selector};
use druid_widget_nursery::StackChildPosition;

use super::Filters;

pub mod filter_button;
pub mod filter_options;

pub const FILTER_POSITION: Selector<Point> = Selector::new("filter_options.position");

const FILTER_WIDTH: f64 = super::CONTROL_WIDTH;

#[derive(Debug, Clone, Data, Lens, Default)]
pub struct FilterState {
  open: bool,
  pub stack_position: StackChildPosition,
  #[data(eq)]
  pub active_filters: HashSet<Filters>,
  #[data(eq)]
  pub sorted_ids: Vec<String>,
}

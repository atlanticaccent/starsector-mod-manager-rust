use druid::{Data, Lens, Point, Selector};
use druid_widget_nursery::StackChildPosition;
use strum::EnumCount;
use types::ArraySet;

use super::Filters;

pub mod filter_button;
pub mod filter_options;

pub const FILTER_POSITION: Selector<Point> = Selector::new("filter_options.position");

const FILTER_WIDTH: f64 = super::CONTROL_WIDTH;
const FILTER_COUNT: usize = <Filters as EnumCount>::COUNT;

#[derive(Debug, Clone, Data, Lens, Default)]
pub struct FilterState {
  open: bool,
  pub stack_position: StackChildPosition,
  #[data(eq)]
  pub active_filters: ArraySet<Filters, FILTER_COUNT>,
}

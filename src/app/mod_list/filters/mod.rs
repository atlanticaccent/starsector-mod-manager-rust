use druid::{Point, Selector};

pub mod filter_button;
pub mod filter_options;

pub const FILTER_POSITION: Selector<Point> = Selector::new("filter_options.position");

const FILTER_WIDTH: f64 = 175.0;

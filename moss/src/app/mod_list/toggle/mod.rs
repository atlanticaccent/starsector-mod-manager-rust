use druid::{Data, Lens};

pub mod toggle_button;
pub mod toggle_options;

pub const INSTALL_WIDTH: f64 = super::CONTROL_WIDTH;

#[derive(Clone, Data, Lens, Default)]
pub struct ToggleState {
  hovered: bool,
  open: bool,
}

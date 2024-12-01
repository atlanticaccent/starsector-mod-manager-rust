use druid::{Data, Lens};

pub mod action_button;
pub mod action_options;

pub const INSTALL_WIDTH: f64 = super::CONTROL_WIDTH;

#[derive(Clone, Data, Lens, Default)]
pub struct ActionsState {
  hovered: bool,
  open: bool,
}

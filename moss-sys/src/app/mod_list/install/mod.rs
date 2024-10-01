use druid::{Data, Lens};

pub mod install_button;
pub mod install_options;

pub const INSTALL_WIDTH: f64 = super::CONTROL_WIDTH;

#[derive(Clone, Data, Lens, Default)]
pub struct InstallState {
  hovered: bool,
  open: bool,
}

use druid::{Data, Lens};

pub mod install_button;
pub mod install_options;

pub const INSTALL_WIDTH: f64 = 175.0;

#[derive(Clone, Data, Lens, Default)]
pub struct InstallState {
  hovered: bool,
  open: bool,
}

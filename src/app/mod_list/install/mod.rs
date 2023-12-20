use druid::{Data, Lens};

pub mod install_button;
pub mod install_options;

#[derive(Clone, Data, Lens, Default)]
pub struct InstallState {
  hovered: bool,
  open: bool,
}

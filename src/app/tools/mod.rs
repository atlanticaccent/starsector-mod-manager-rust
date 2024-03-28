use std::path::PathBuf;

use druid::{
  widget::{Flex, Maybe},
  Data, Lens, Widget, WidgetExt,
};
use druid_widget_nursery::WidgetExt as _;

use crate::widgets::card::{Card, CardBuilder};

use self::{jre::Swapper, vmparams::VMParams};

use super::util::WidgetExtEx;

pub mod jre;
pub mod vmparams;

#[derive(Debug, Clone, Data, Lens)]
pub struct Tools {
  #[data(eq)]
  pub install_path: Option<PathBuf>,
  pub vmparams: Option<VMParams>,
}

impl Tools {
  pub fn view() -> impl Widget<Self> {
    Flex::column()
      .must_fill_main_axis(true)
      .with_child(Self::vmparams_wrapped())
      .with_default_spacer()
      .with_child(Self::jre_swapper())
  }

  fn vmparams_wrapped() -> impl Widget<Self> {
    Maybe::or_empty(|| VMParams::view())
      .lens(Tools::vmparams)
      .on_change(|_, _, data, _| data.write_vmparams())
      .on_command(VMParams::TOGGLE_UNIT_DROP, |_, payload, data| {
        if let Some(vmparams) = data.vmparams.as_mut()
          && vmparams.linked
          && !*payload
        {
          vmparams.heap_max = vmparams.heap_init.clone()
        }
        data.write_vmparams()
      })
  }

  fn jre_swapper() -> impl Widget<Self> {
    Swapper::view()
      .scope_independent(|| Swapper {
        current_flavour: jre::Flavour::Original,
        jre_swap_in_progress: false,
    })
  }

  fn write_vmparams(&self) {
    if let Some(install) = self.install_path.as_ref()
      && let Some(vmparams) = &self.vmparams
    {
      vmparams.save(install).expect("Save vmparams edit")
    }
  }
}

pub fn tool_card<T: Data>() -> CardBuilder<T> {
  Card::builder()
    .with_insets((0.0, 14.0))
    .with_corner_radius(4.0)
    .with_shadow_length(6.0)
}

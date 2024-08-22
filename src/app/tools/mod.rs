use std::path::PathBuf;

use druid::{
  widget::{Flex, Maybe, SizedBox, ViewSwitcher},
  Data, Lens, Widget, WidgetExt,
};
use druid_widget_nursery::{FutureWidget, WidgetExt as _};
use proc_macros::Invert;

use self::{jre::Swapper, vmparams::VMParams};
use super::{
  settings::Settings,
  util::{LensExtExt, WidgetExtEx},
};
use crate::widgets::card::{Card, CardBuilder};

pub mod jre;
pub mod vmparams;

#[derive(Debug, Clone, Data, Lens, Invert)]
pub struct Tools {
  #[data(eq)]
  pub install_path: Option<PathBuf>,
  pub vmparams: Option<VMParams>,
}

impl Tools {
  pub fn settings_sync() -> impl Lens<Settings, Tools> {
    druid::lens::Map::new(
      |settings: &Settings| Tools {
        vmparams: settings.vmparams.clone().map(|mut v| {
          v.linked = settings.vmparams_linked;
          v
        }),
        install_path: settings.install_dir.clone(),
      },
      |settings, tools| {
        settings.vmparams = tools.vmparams;
        settings.vmparams_linked = settings
          .vmparams
          .as_ref()
          .map(|v| v.linked)
          .unwrap_or_default()
      },
    )
  }

  pub fn view() -> impl Widget<Self> {
    Flex::column()
      .must_fill_main_axis(true)
      .with_child(Self::vmparams_wrapped())
      .with_default_spacer()
      .with_child(Self::jre_swapper())
  }

  fn vmparams_wrapped() -> impl Widget<Self> {
    Maybe::or_empty(VMParams::view)
      .lens(Tools::vmparams)
      .on_command(VMParams::SAVE_VMPARAMS, |_, _, data| {
        eprintln!("saving vmparams");
        data.write_vmparams()
      })
  }

  fn jre_swapper() -> impl Widget<Self> {
    #[derive(Clone, Data)]
    struct PathWrapper(#[data(eq)] PathBuf);

    ViewSwitcher::new(
      |data: &Option<PathWrapper>, _| data.clone(),
      |_, _, _| {
        Maybe::or_empty(|| {
          FutureWidget::new(
            |data: &PathWrapper, _| Swapper::get_cached_jres(data.0.clone()),
            SizedBox::empty(),
            |res, data, _| {
              let (current_flavour, cached_flavours) = *res;
              let cached_flavours: druid::im::Vector<_> = cached_flavours.into();
              let install_dir = data.0.clone();
              Swapper::view()
                .scope_independent(move || Swapper {
                  current_flavour,
                  cached_flavours: cached_flavours.clone(),
                  install_dir: install_dir.clone(),
                  jre_23: false,
                })
                .boxed()
            },
          )
        })
        .boxed()
      },
    )
    .lens(Tools::install_path.compute(|p| p.clone().map(PathWrapper)))
  }

  fn write_vmparams(&self) {
    if let Some(install) = self.install_path.as_ref()
      && let Some(vmparams) = &self.vmparams
    {
      vmparams.save(install).expect("Save vmparams edit")
    }
  }
}

pub fn tool_card() -> CardBuilder {
  Card::builder()
    .with_insets((0.0, 14.0))
    .with_corner_radius(4.0)
    .with_shadow_length(6.0)
}

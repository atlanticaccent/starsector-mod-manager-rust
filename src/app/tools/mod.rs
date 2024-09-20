use std::path::PathBuf;

use derive_more::derive::{From, Into};
use druid::{
  widget::{Flex, Maybe, SizedBox, ViewSwitcher},
  Data, Lens, LensExt, Widget, WidgetExt,
};
use druid_widget_nursery::{FutureWidget, WidgetExt as _};
use proc_macros::Invert;

use self::{jre::Swapper, vmparams::VMParams};
use super::settings::Settings;
use crate::{
  app::util::{Convert, LensExtExt, WidgetExtEx},
  widgets::card::{Card, CardBuilder},
};

pub mod jre;
pub mod vmparams;

#[Invert]
#[derive(Debug, Clone, Data, Lens)]
pub struct Tools {
  #[data(eq)]
  pub install_dir: Option<PathBuf>,
  pub(crate) vmparams: Option<VMParams>,
  vmparams_linked: bool,
  jre_23: bool,
}

impl Tools {
  pub fn settings_sync() -> impl Lens<Settings, Tools> {
    druid::lens::Map::new(|settings| settings.into(), assign_settings)
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
    #[derive(Clone, Data, From, Into, Default)]
    struct PathWrapper(#[data(eq)] PathBuf);

    ViewSwitcher::new(
      |data: &Option<InstallDirInverseTools>, _| {
        data
          .as_ref()
          .map(|inner| PathWrapper(inner.install_dir.clone()))
      },
      |_, _, _| {
        Maybe::or_empty(|| {
          FutureWidget::new(
            |data: &InstallDirInverseTools, _| Swapper::get_cached_jres(data.install_dir.clone()),
            SizedBox::empty(),
            |res, _data, _| {
              #[cfg_attr(target_os = "macos", allow(unused_mut))]
              let (mut current_flavour, cached_flavours) = *res;
              let cached_flavours: druid::im::Vector<_> = cached_flavours.into();
              #[cfg(not(target_os = "macos"))]
              if _data.jre_23 && cached_flavours.contains(&jre::Flavour::Miko) {
                current_flavour = jre::Flavour::Miko;
              }

              Swapper::view()
                .partial_scope(
                  move |tools: InstallDirInverseTools| Swapper {
                    current_flavour,
                    cached_flavours,
                    install_dir: tools.install_dir.clone(),
                    jre_23: tools.jre_23,
                  },
                  (
                    Swapper::install_dir.then(Convert::<PathBuf, PathWrapper>::new()),
                    Swapper::jre_23,
                  ),
                  (
                    InstallDirInverseTools::install_dir.convert::<PathWrapper>(),
                    InstallDirInverseTools::jre_23,
                  ),
                )
                .boxed()
            },
          )
        })
        .boxed()
      },
    )
    .lens(Tools::invert_on_install_dir)
  }

  fn write_vmparams(&self) {
    if let Some(install) = self.install_dir.as_ref()
      && let Some(vmparams) = &self.vmparams
    {
      vmparams.save(install).expect("Save vmparams edit")
    }
  }
}

impl<'a> From<&'a Settings> for Tools {
  fn from(settings: &'a Settings) -> Self {
    Self {
      install_dir: settings.install_dir.clone(),
      vmparams: settings.vmparams.clone(),
      vmparams_linked: settings.vmparams_linked,
      jre_23: settings.jre_23,
    }
  }
}

fn assign_settings(
  settings: &mut Settings,
  Tools {
    install_dir: _,
    vmparams,
    vmparams_linked,
    jre_23,
  }: Tools,
) {
  settings.vmparams = vmparams;
  settings.vmparams_linked = vmparams_linked;
  settings.jre_23 = jre_23;
}

pub fn tool_card() -> CardBuilder {
  Card::builder()
    .with_insets((0.0, 14.0))
    .with_corner_radius(4.0)
    .with_shadow_length(6.0)
}

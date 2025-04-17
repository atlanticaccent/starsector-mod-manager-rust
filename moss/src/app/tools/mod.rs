use std::path::PathBuf;

use common::{
  lenses::LensExtExt,
  widget_ext::WidgetExtEx,
  widgets::card::{Card, CardBuilder},
};
use derive_more::derive::{From, Into};
use druid::{
  widget::{Flex, Maybe, SizedBox, ViewSwitcher},
  Data, Lens, Widget, WidgetExt,
};
use druid_widget_nursery::{FutureWidget, WidgetExt as _};
use macros::OptionSpec;

use self::{jre::Swapper, vmparams::VMParams};
use crate::app::{mod_entry::GameVersion, App};

pub mod jre;
pub mod vmparams;

#[OptionSpec]
#[derive(Debug, Clone, Data, Lens)]
pub struct Tools {
  #[data(eq)]
  pub install_dir: Option<PathBuf>,
  pub(crate) vmparams: Option<VMParams>,
  vmparams_linked: bool,
  jre_23: bool,
  game_version: Option<GameVersion>,
}

impl Tools {
  pub fn settings_sync() -> impl Lens<App, Tools> {
    druid::lens::Map::new(|settings| settings.into(), assign_settings)
  }

  pub fn view() -> impl Widget<Self> {
    Flex::column()
      .must_fill_main_axis(true)
      .with_child(Self::vmparams_wrapped())
      .with_default_spacer()
      .with_child(Self::jre_swapper().empty_if_not(|data, _| {
        data
          .game_version
          .as_ref()
          .is_some_and(|(major, minor, ..)| {
            major.as_deref() == Some("0") && minor.as_deref() == Some("97")
          })
      }))
  }

  fn vmparams_wrapped() -> impl Widget<Self> {
    Maybe::or_empty(VMParams::view)
      .lens(Tools::vmparams)
      .on_command(VMParams::SAVE_VMPARAMS, |_, (), data| {
        eprintln!("saving vmparams");
        data.write_vmparams();
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
                    Swapper::install_dir.convert::<PathWrapper>(),
                    Swapper::jre_23,
                  ),
                  (
                    InstallDirInverseTools::install_dir.convert(),
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
      vmparams.save(install).expect("Save vmparams edit");
    }
  }
}

impl<'a> From<&'a App> for Tools {
  fn from(app: &'a App) -> Self {
    Self {
      install_dir: app.settings.install_dir.clone(),
      vmparams: app.settings.vmparams.clone(),
      vmparams_linked: app.settings.vmparams_linked,
      jre_23: app.settings.jre_23,
      game_version: app.mod_list.starsector_version.clone(),
    }
  }
}

fn assign_settings(
  app: &mut App,
  Tools {
    install_dir: _,
    vmparams,
    vmparams_linked,
    jre_23,
    game_version: _,
  }: Tools,
) {
  app.settings.vmparams = vmparams;
  app.settings.vmparams_linked = vmparams_linked;
  app.settings.jre_23 = jre_23;
}

pub fn tool_card() -> CardBuilder {
  Card::builder()
    .with_insets((0.0, 14.0))
    .with_corner_radius(4.0)
    .with_shadow_length(6.0)
}

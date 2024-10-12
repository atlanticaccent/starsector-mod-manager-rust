use std::ops::Deref;

use druid::Data;
use druid_widget_nursery::material_icons::IconPaths;

#[derive(Clone, Data)]
pub struct Icon {
  #[data(ignore)]
  inner: IconPaths,
  id: &'static str,
  color: Option<druid::Color>,
}

impl Icon {
  pub const fn new(inner: IconPaths, id: &'static str) -> Self {
    Self {
      inner,
      id,
      color: None,
    }
  }

  pub fn with_color(mut self, color: druid::Color) -> Self {
    self.color = Some(color);
    self
  }

  pub fn color(&self) -> &Option<druid::Color> {
    &self.color
  }
}

impl Deref for Icon {
  type Target = IconPaths;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl PartialEq for Icon {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
  }
}

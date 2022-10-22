use std::rc::Rc;

use druid::{
  theme,
  widget::{LabelText, SizedBox, TabInfo, TabsPolicy},
  Data, KeyOrValue, SingleUse, UnitPoint, Widget, WidgetExt,
};

/// A TabsPolicy that allows the app developer to provide static tabs up front when building the
/// widget.
#[derive(Clone)]
pub struct StaticTabsForked<T> {
  // This needs be able to avoid cloning the widgets we are given -
  // as such it is Rc
  tabs: Rc<[InitialTab<T>]>,
  text_size: KeyOrValue<f64>,
  label_height: f64,
}

#[allow(dead_code)]
impl<T> StaticTabsForked<T> {
  /// Set the static tabs forked's text size.
  pub fn set_text_size(mut self, text_size: KeyOrValue<f64>) -> Self {
    self.text_size = text_size;
    self
  }

  /// Set the static tabs forked's label height.
  pub fn set_label_height(mut self, label_height: f64) -> Self {
    self.label_height = label_height;
    self
  }
}

impl<T> Default for StaticTabsForked<T> {
  fn default() -> Self {
    StaticTabsForked {
      tabs: Rc::new([]),
      text_size: theme::TEXT_SIZE_NORMAL.into(),
      label_height: 18.,
    }
  }
}

impl<T: Data> Data for StaticTabsForked<T> {
  fn same(&self, _other: &Self) -> bool {
    // Changing the tabs after construction shouldn't be possible for static tabs
    true
  }
}

impl<T: Data> TabsPolicy for StaticTabsForked<T> {
  type Key = usize;
  type Input = T;
  type BodyWidget = Box<dyn Widget<T>>;
  type LabelWidget = SizedBox<T>;
  type Build = Vec<InitialTab<T>>;

  fn tabs_changed(&self, _old_data: &T, _data: &T) -> bool {
    false
  }

  fn tabs(&self, _data: &T) -> Vec<Self::Key> {
    (0..self.tabs.len()).collect()
  }

  fn tab_info(&self, key: Self::Key, _data: &T) -> TabInfo<Self::Input> {
    // This only allows a static tabs label to be retrieved once,
    // but as we never indicate that the tabs have changed,
    // it should only be called once per key.
    TabInfo::new(
      self.tabs[key]
        .name
        .take()
        .expect("StaticTabs LabelText can only be retrieved once"),
      false,
    )
  }

  fn tab_body(&self, key: Self::Key, _data: &T) -> Self::BodyWidget {
    // This only allows a static tab to be retrieved once,
    // but as we never indicate that the tabs have changed,
    // it should only be called once per key.
    self
      .tabs
      .get(key)
      .and_then(|initial_tab| initial_tab.child.take())
      .expect("StaticTabs body widget can only be retrieved once")
  }

  fn tab_label(
    &self,
    _key: Self::Key,
    info: TabInfo<Self::Input>,
    _data: &Self::Input,
  ) -> Self::LabelWidget {
    Self::default_make_label(info)
      .with_text_size(self.text_size.clone())
      .align_vertical(UnitPoint::CENTER)
      .fix_height(self.label_height)
  }

  fn build(build: Self::Build) -> Self {
    StaticTabsForked {
      tabs: build.into(),
      text_size: theme::TEXT_SIZE_NORMAL.into(),
      label_height: 18.,
    }
  }
}

pub struct InitialTab<T> {
  name: SingleUse<LabelText<T>>, // This is to avoid cloning provided label texts
  child: SingleUse<Box<dyn Widget<T>>>, // This is to avoid cloning provided tabs
}

impl<T: Data> InitialTab<T> {
  pub fn new(name: impl Into<LabelText<T>>, child: impl Widget<T> + 'static) -> Self {
    InitialTab {
      name: SingleUse::new(name.into()),
      child: SingleUse::new(child.boxed()),
    }
  }
}

#![feature(trait_alias)]
#![feature(let_chains)]
#![feature(type_alias_impl_trait)]

pub mod controllers;
pub mod fast_im_map;
pub mod labels;
pub mod lenses;
#[allow(unused_macros)]
pub mod macro_rules;
pub mod theme_keys;
pub mod widget_ext;
#[allow(dead_code)]
pub mod widgets;

use std::any::Any;

use druid::{lens, Color, Event, ExtEventError, ExtEventSink, Key, MouseEvent, Selector, Target};
use druid_widget_nursery::animation::Interpolate as _;

pub const IS_EMPTY: Selector = Selector::new("app.popup.empty");
pub const SHADOW: Key<Color> = Key::new("custom_theme.shadow");

pub trait ShadeColor {
  fn lighter(self) -> Self;

  fn lighter_by(self, mult: usize) -> Self;

  fn darker(self) -> Self;

  fn darker_by(self, mult: usize) -> Self;

  fn interpolate_with(self, other: Self, mult: usize) -> Self;
}

impl ShadeColor for Color {
  fn lighter(self) -> Self {
    self.interpolate(&Color::WHITE, 1.0 / 16.0)
  }

  fn lighter_by(self, mult: usize) -> Self {
    self.interpolate(&Color::WHITE, mult as f64 / 16.0)
  }

  fn darker(self) -> Self {
    self.interpolate(&Color::BLACK, 1.0 / 16.0)
  }

  fn darker_by(self, mult: usize) -> Self {
    self.interpolate(&Color::BLACK, mult as f64 / 16.0)
  }

  fn interpolate_with(self, other: Self, mult: usize) -> Self {
    self.interpolate(&other, mult as f64 / 16.)
  }
}

#[extend::ext(name = EventExt)]
pub impl Event {
  fn get_cmd<T: 'static>(&self, selector: Selector<T>) -> Option<&T> {
    if let Event::Command(cmd) = self {
      cmd.get(selector)
    } else {
      None
    }
  }

  fn is_cmd<T: 'static>(&self, selector: Selector<T>) -> bool {
    if let Event::Command(cmd) = self {
      cmd.is(selector)
    } else {
      false
    }
  }

  fn as_mouse_up(&self) -> Option<&MouseEvent> {
    if let Event::MouseUp(mouse) = self {
      Some(mouse)
    } else {
      None
    }
  }
}

pub trait ExtEventSinkExt {
  fn submit_command_global<T: Any + Send>(
    &self,
    selector: Selector<T>,
    payload: impl Into<Box<T>>,
  ) -> Result<(), ExtEventError>;
}

impl ExtEventSinkExt for ExtEventSink {
  fn submit_command_global<T: Any + Send>(
    &self,
    selector: Selector<T>,
    payload: impl Into<Box<T>>,
  ) -> Result<(), ExtEventError> {
    self.submit_command(selector, payload, Target::Global)
  }
}

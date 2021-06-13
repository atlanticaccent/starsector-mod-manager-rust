pub mod button_none {
  use iced::{button, Color, Vector};

  pub struct Button;

  impl button::StyleSheet for Button {
    fn active(&self) -> button::Style {
      button::Style {
        background: None,
        shadow_offset: Vector::new(0.0, 0.0),
        text_color: Color::from_rgb(0.0, 0.0, 0.0),
        ..button::Style::default()
      }
    }
  
    fn hovered(&self) -> button::Style {
      button::Style {
        border_color: Color::BLACK,
        border_width: 1.0,
        ..self.active()
      }
    }
  }
}

pub mod button_only_hover {
  use iced::{button, Color, Vector};

  pub struct Button;

  impl button::StyleSheet for Button {
    fn active(&self) -> button::Style {
      button::Style {
        shadow_offset: Vector::new(0.0, 0.0),
        text_color: Color::WHITE.into(),
        ..button::Style::default()
      }
    }
  
    fn hovered(&self) -> button::Style {
      button::Style {
        background: Color::from_rgb8(0x41, 0x41, 0x41).into(),
        text_color: Color::WHITE.into(),
        ..button::Style::default()
      }
    }
  }
}

pub mod nav_bar {
  use iced::{container, Color};

  pub struct Container;

  impl container::StyleSheet for Container {
    fn style(&self) -> container::Style {
      container::Style {
        background: Color::from_rgb8(0x12, 0x12, 0x12).into(),
        text_color: Color::WHITE.into(),
        ..container::Style::default()
      }
    }
  }
}

pub mod max_rule {
  use iced::rule;

  pub struct Rule;

  impl rule::StyleSheet for Rule {
    fn style(&self) -> rule::Style {
      rule::Style {
        color: [0.6, 0.6, 0.6, 0.51].into(),
        width: 1,
        radius: 0.0,
        fill_mode: rule::FillMode::Full
      }
    }
  }
}

pub mod alternate_background {
  use iced::{container, Color};

  pub struct Container;

  impl container::StyleSheet for Container {
    fn style(&self) -> container::Style {
      container::Style {
        background: Color::from_rgb8(0xF2, 0xF2, 0xF2).into(),
        ..container::Style::default()
      }
    }
  }
}

pub mod highlight_background {
  use iced::{container, Color};

  pub struct Container;

  impl container::StyleSheet for Container {
    fn style(&self) -> container::Style {
      container::Style {
        background: Color::from_rgb8(0xDF, 0xFB, 0xF8).into(),
        ..container::Style::default()
      }
    }
  }
}

use iced::container;
use crate::gui::mod_list::{UpdateStatus, UpdateStatusTTPatch};

impl From<UpdateStatus> for Box<dyn container::StyleSheet> {
  fn from(theme: UpdateStatus) -> Self {
    match theme {
      UpdateStatus::Major(_) | UpdateStatus::Minor(_) | UpdateStatus::Patch(_) => update::major::Container.into(),
      UpdateStatus::UpToDate => update::up_to_date::Container.into(),
      UpdateStatus::Error => update::error::Container.into()
    }
  }
}

impl From<UpdateStatusTTPatch> for Box<dyn container::StyleSheet> {
  fn from(wrapper: UpdateStatusTTPatch) -> Self {
    let UpdateStatusTTPatch(theme) = wrapper;
    match theme {
      UpdateStatus::Major(_) | UpdateStatus::Minor(_) | UpdateStatus::Patch(_) => update::major::Tooltip.into(),
      UpdateStatus::UpToDate => update::up_to_date::Tooltip.into(),
      UpdateStatus::Error => update::error::Tooltip.into()
    }
  }
}

pub mod update {
  use iced::{container, Color};

  pub mod major {
    pub struct Container;
    pub struct Tooltip;

    fn style() -> super::container::Style {
      super::container::Style {
        background: super::Color::from_rgb8(0xFF, 0xA0, 0x00).into(),
        ..super::container::Style::default()
      }
    }

    impl super::container::StyleSheet for Container {
      fn style(&self) -> super::container::Style {
        style()
      }
    }

    impl super::container::StyleSheet for Tooltip {
      fn style(&self) -> super::container::Style {
        super::container::Style {
          border_color: super::Color::BLACK,
          border_width: 1.0,
          border_radius: 5.0,
          ..style()
        }
      }
    }
  }

  pub mod up_to_date {
    pub struct Container;
    pub struct Tooltip;

    fn style() -> super::container::Style {
      super::container::Style {
        background: super::Color::from_rgb8(0x03, 0x9B, 0xE5).into(),
        ..super::container::Style::default()
      }
    }

    impl super::container::StyleSheet for Container {
      fn style(&self) -> super::container::Style {
        style()
      }
    }

    impl super::container::StyleSheet for Tooltip {
      fn style(&self) -> super::container::Style {
        super::container::Style {
          border_color: super::Color::BLACK,
          border_width: 1.0,
          border_radius: 5.0,
          ..style()
        }
      }
    }
  }

  pub mod error {
    pub struct Container;
    pub struct Tooltip;

    fn style() -> super::container::Style {
      super::container::Style {
        background: super::Color::from_rgb8(0xB0, 0x00, 0x20).into(),
        text_color: Some(super::Color::WHITE),
        ..super::container::Style::default()
      }
    }

    impl super::container::StyleSheet for Container {
      fn style(&self) -> super::container::Style {
        style()
      }
    }

    impl super::container::StyleSheet for Tooltip {
      fn style(&self) -> super::container::Style {
        super::container::Style {
          border_color: super::Color::BLACK,
          border_width: 1.0,
          border_radius: 5.0,
          ..style()
        }
      }
    }
  }
}

pub mod hyperlink_block {
  use iced::{button, Color, Vector};

  pub struct Button;

  impl button::StyleSheet for Button {
    fn active(&self) -> button::Style {
      button::Style {
        background: None,
        shadow_offset: Vector::new(0.0, 0.0),
        text_color: Color::from_rgb8(0x06, 0x45, 0xAD),
        ..button::Style::default()
      }
    }
  
    fn hovered(&self) -> button::Style {
      button::Style {
        background: Color::from_rgb8(0x06, 0x45, 0xAD).into(),
        text_color: Color::WHITE,
        ..self.active()
      }
    }
  }
}

pub mod button_none {
  use iced::{button, Color, Vector};

  pub struct Button;

  impl button::StyleSheet for Button {
    fn active(&self) -> button::Style {
      button::Style {
        background: Color::from_rgb(255.0, 255.0, 255.0).into(),
        shadow_offset: Vector::new(0.0, 0.0),
        text_color: Color::from_rgb(0.0, 0.0, 0.0),
        ..button::Style::default()
      }
    }
  
    fn hovered(&self) -> button::Style {
      button::Style {
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

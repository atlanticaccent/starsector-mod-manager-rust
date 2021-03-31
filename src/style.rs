pub mod none {
  use iced::{button, Color, Vector};

  pub struct Button;

  impl button::StyleSheet for Button {
    fn active(&self) -> button::Style {
      button::Style {
        background: Color::from_rgb(255.0, 255.0, 255.0).into(),
        border_radius: 12.0,
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
        background: Color::from_rgb(255.0, 255.0, 255.0).into(),
        shadow_offset: Vector::new(0.0, 0.0),
        text_color: Color::from_rgb(0.0, 0.0, 0.0),
        ..button::Style::default()
      }
    }
  
    fn hovered(&self) -> button::Style {
      button::Style {
        background: Color::from_rgb8(214, 234, 248).into(),
        ..button::Style::default()
      }
    }
  }
}

pub mod border {
  use iced::{container, Color};

  pub struct Container;

  impl container::StyleSheet for Container {
    fn style(&self) -> container::Style {
      container::Style {
        border_width: 0.5,
        border_color: Color::from_rgb(0.0, 0.0, 0.0),
        ..container::Style::default()
      }
    }
  }
}

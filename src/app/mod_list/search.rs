use druid::{
  lens,
  widget::{Flex, TextBox},
  Data, Lens, UnitPoint, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, Stack, StackChildPosition, WidgetExt as _};

use crate::{
  app::util::{WidgetExtEx, WithHoverState, CANCEL, SEARCH},
  widgets::card::Card,
};

#[derive(Debug, Clone, Data, Lens)]
pub struct Search {
  buffer: String,
}

impl Search {
  pub fn new(buffer: String) -> Self {
    Self { buffer }
  }

  pub fn len(&self) -> usize {
    self.buffer.len()
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn view() -> impl Widget<Self> {
    Card::new(
      Stack::new()
        .with_child(
          Flex::row()
            .with_child(Icon::new(*SEARCH).padding((5.0, 0.0)))
            .with_flex_child(
              TextBox::new().with_placeholder("Search").expand_width(),
              1.0,
            )
            .expand_width(),
        )
        .with_positioned_child(
          Icon::new(*CANCEL)
            .env_scope(|env, _| {
              env.set(
                druid::theme::TEXT_COLOR,
                env.get(druid::theme::TEXT_COLOR).with_alpha(0.5),
              )
            })
            .invisible_if(String::is_empty)
            .suppress_event(|event| matches!(event, druid::Event::MouseMove(_)))
            .lens(lens!((String, bool), 0))
            .with_hover_state(true)
            .on_click(|_, data, _| data.clear())
            .disabled_if(|t, _| t.is_empty()),
          StackChildPosition::new().right(Some(0.0)),
        )
        .align(UnitPoint::RIGHT),
    )
    .env_scope(|env, _| env.set(druid::theme::BORDER_DARK, druid::Color::TRANSPARENT))
    .lens(Self::buffer)
    .on_command(
      super::install::install_options::InstallOptions::DISMISS,
      |ctx, point, _| {
        let hitbox = ctx
          .size()
          .to_rect()
          .with_origin(ctx.to_window((0.0, 0.0).into()));
        if !hitbox.contains(*point) && ctx.has_focus() {
          ctx.resign_focus()
        }
      },
    )
    .on_command(crate::app::App::DUMB_UNIVERSAL_ESCAPE, |ctx, _, _| {
      if ctx.has_focus() {
        ctx.resign_focus()
      }
    })
    .on_key_up(druid::keyboard_types::Key::Enter, |ctx, _| {
      ctx.resign_focus();
      true
    })
    .fix_size(250.0, 52.0)
  }
}
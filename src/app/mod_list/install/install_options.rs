use druid::{theme, Widget, WidgetExt};

use super::{install_button::InstallButton, InstallState};
use crate::{
  app::util::{bold_text, WidgetExtEx},
  widgets::card::Card,
};

pub struct InstallOptions;

impl InstallOptions {
  pub fn view() -> impl Widget<InstallState> {
    let text = |text| {
      bold_text(
        text,
        druid::theme::TEXT_SIZE_NORMAL,
        druid::FontWeight::SEMI_BOLD,
        druid::theme::TEXT_COLOR,
      )
      .padding((8.0, 0.0))
    };

    let mut width_linker = None;
    Card::builder()
      .with_insets((0.0, 14.0))
      .with_corner_radius(4.0)
      .with_shadow_length(8.0)
      .with_background(theme::BACKGROUND_DARK)
      .build(
        InstallButton::button_styling(
          InstallButton::inner(true)
            .with_spacer(4.0)
            .with_child(
              Card::hoverable(|| text("From Archive"), (0.0, 10.0))
                .link_height_with(&mut width_linker)
                .horizontal()
                .on_click(|_, data: &mut InstallState, _| data.open = false),
            )
            .with_child(
              Card::hoverable(|| text("From Folder"), (0.0, 10.0))
                .link_height_with(&mut width_linker)
                .on_click(|_, data, _| data.open = false),
            ),
        )
        .fix_height(128.0),
      )
      .or_empty(|data: &InstallState, _| data.open)
  }
}

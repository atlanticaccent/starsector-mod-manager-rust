use druid::{theme, Widget, WidgetExt, Selector, Point};
use druid_widget_nursery::WidgetExt as _;

use super::{install_button::InstallButton, InstallState};
use crate::{
  app::{util::{bold_text, WidgetExtEx}, App, app_delegate::AppCommands},
  widgets::card::Card,
};

pub struct InstallOptions;

impl InstallOptions {
  pub const DISMISS: Selector<Point> = Selector::new("install_options.dismiss");

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
                .on_click(|ctx, data: &mut InstallState, _| {
                  data.open = false;
                  ctx.submit_command(App::SELECTOR.with(AppCommands::PickFile(true)))
                }),
            )
            .with_child(
              Card::hoverable(|| text("From Folder"), (0.0, 10.0))
                .link_height_with(&mut width_linker)
                .on_click(|ctx, data, _| {
                  data.open = false;
                  ctx.submit_command(App::SELECTOR.with(AppCommands::PickFile(false)))
                }),
            ),
        )
        .fix_height(128.0),
      )
      .or_empty(|data: &InstallState, _| data.open)
      .on_command(Self::DISMISS, |ctx, payload, data| {
        let hitbox = ctx.size().to_rect().with_origin(ctx.to_window((0.0, 0.0).into()));
        if !hitbox.contains(*payload) {
          data.open = false
        }
      })
  }
}

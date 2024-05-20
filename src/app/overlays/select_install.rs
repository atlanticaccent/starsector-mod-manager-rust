use druid::{
  text::ParseFormatter,
  widget::{Button, Flex, TextBox},
  LensExt, Selector, Widget, WidgetExt as _,
};
use druid_widget_nursery::WidgetExt;

use super::Popup;
use crate::{
  app::{
    controllers::HoverController,
    settings::{InstallDirDelegate, Settings, SettingsCommand},
    util::{h2_fixed, CommandExt as _},
    App,
  },
  widgets::card::Card,
};

pub struct SelectInstall;

impl SelectInstall {
  #[cfg(target_os = "macos")]
  const HEADING: &'static str = "Please select the Starsector app.";
  #[cfg(not(target_os = "macos"))]
  const HEADING: &'static str = "Please select your Starsector installation directory.";

  pub fn view() -> impl Widget<App> {
    Flex::row()
      .with_flex_spacer(1.)
      .with_flex_child(
        Card::builder()
          .with_insets(Card::CARD_INSET)
          .with_background(druid::theme::BACKGROUND_LIGHT)
          .build(
            Flex::column()
              .with_child(h2_fixed(SelectInstall::HEADING))
              .with_child(
                Flex::row()
                  .with_flex_child(
                    TextBox::multiline()
                      .with_line_wrapping(true)
                      .with_formatter(ParseFormatter::new())
                      .delegate(InstallDirDelegate)
                      .lens(App::settings.then(Settings::install_dir_buf))
                      .expand_width(),
                    1.,
                  )
                  .with_child(
                    Button::new("Browse...")
                      .controller(HoverController::default())
                      .on_click(|ctx, _, _| {
                        ctx.submit_command_global(Selector::new(
                          "druid.builtin.textbox-cancel-editing",
                        ));
                        ctx.submit_command_global(
                          Settings::SELECTOR.with(SettingsCommand::SelectInstallDir),
                        )
                      }),
                  ),
              ),
          ),
        2.,
      )
      .with_flex_spacer(1.)
      .on_command(Settings::SELECTOR, |ctx, payload, _| {
        if let SettingsCommand::UpdateInstallDir(_) = payload {
          ctx.submit_command(Popup::DISMISS);
        }
      })
  }
}

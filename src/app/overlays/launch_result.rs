use druid::{
  widget::{Flex, Label},
  Data, Key, Widget, WidgetExt,
};

use super::Popup;
use crate::{
  app::util::{h2_fixed, WidgetExtEx as _},
  patch::table::{FixedFlexTable, TableRow},
  theme::{BLUE_KEY, ON_BLUE_KEY},
  widgets::card::Card,
};

pub struct LaunchResult;

impl LaunchResult {
  pub fn view<T: Data>(error: String) -> impl Widget<T> {
    Flex::row()
      .with_flex_spacer(0.5)
      .with_flex_child(
        Card::builder()
          .with_insets(Card::CARD_INSET)
          .with_background(druid::theme::BACKGROUND_LIGHT)
          .build(
            FixedFlexTable::new()
              .with_row(
                TableRow::new().with_child(h2_fixed("Could not launch Starsector").halign_centre()),
              )
              .with_row(TableRow::new().with_child(Label::new("Error:").align_left()))
              .with_row(TableRow::new().with_child(Label::new(error).align_left()))
              .with_row(
                TableRow::new().with_child(
                  Card::builder()
                    .with_insets((0.0, 8.0))
                    .with_corner_radius(6.0)
                    .with_shadow_length(2.0)
                    .with_shadow_increase(2.0)
                    .with_border(2.0, Key::new("button.border"))
                    .hoverable(|_| {
                      Flex::row()
                        .with_child(Label::new("Continue").padding((10.0, 0.0)))
                        .valign_centre()
                    })
                    .env_scope(|env, _| {
                      env.set(druid::theme::BACKGROUND_LIGHT, env.get(BLUE_KEY));
                      env.set(druid::theme::TEXT_COLOR, env.get(ON_BLUE_KEY));
                      env.set(
                        Key::<druid::Color>::new("button.border"),
                        env.get(ON_BLUE_KEY),
                      );
                    })
                    .fix_height(42.0)
                    .padding((0.0, 2.0))
                    .on_click(move |ctx, _, _| {
                      ctx.submit_command(Popup::DISMISS);
                    })
                    .align_right(),
                ),
              ),
          ),
        1.0,
      )
      .with_flex_spacer(0.5)
  }
}

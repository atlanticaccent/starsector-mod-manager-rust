use druid::{
  widget::{Flex, Label},
  Data, Key, Widget, WidgetExt,
};

use crate::{
  app::{
    mod_entry::ModEntry,
    util::{h2_fixed, WidgetExtEx as _, GREEN_KEY, ON_GREEN_KEY, ON_RED_KEY, RED_KEY},
  },
  widgets::card::Card,
};

use super::Popup;

pub struct ConfirmDelete;

impl ConfirmDelete {
  pub fn view<T: Data>(entry: &ModEntry) -> impl Widget<T> {
    Card::builder()
      .with_insets(Card::CARD_INSET)
      .with_background(druid::theme::BACKGROUND_LIGHT)
      .build(
        Flex::column()
          .with_child(h2_fixed(&format!(
            r#"Are you sure you want to delete "{}"?"#,
            entry.name
          )))
          .with_child(Label::new("This action is permanent and cannot be undone."))
          .with_child(
            Flex::row().with_child(
              Card::builder()
                .with_insets((0.0, 8.0))
                .with_corner_radius(6.0)
                .with_shadow_length(2.0)
                .with_shadow_increase(2.0)
                .with_border(2.0, Key::new("button.border"))
                .hoverable(|| {
                  Flex::row()
                    .with_child(Label::new("OK").padding((10.0, 0.0)))
                    .align_vertical_centre()
                })
                .env_scope(|env, _| {
                  env.set(druid::theme::BACKGROUND_LIGHT, env.get(GREEN_KEY));
                  env.set(druid::theme::TEXT_COLOR, env.get(ON_GREEN_KEY));
                  env.set(
                    Key::<druid::Color>::new("enabled_card.border"),
                    env.get(ON_GREEN_KEY),
                  );
                })
                .fix_height(42.0)
                .padding((0.0, 2.0))
                .on_click(|ctx, _, _| ctx.submit_command(Popup::DISMISS)),
            ),
          )
          .with_child(
            Flex::row().with_child(
              Card::builder()
                .with_insets((0.0, 8.0))
                .with_corner_radius(6.0)
                .with_shadow_length(2.0)
                .with_shadow_increase(2.0)
                .with_border(2.0, Key::new("button.border"))
                .hoverable(|| {
                  Flex::row()
                    .with_child(Label::new("Cancel").padding((10.0, 0.0)))
                    .align_vertical_centre()
                })
                .env_scope(|env, _| {
                  env.set(druid::theme::BACKGROUND_LIGHT, env.get(RED_KEY));
                  env.set(druid::theme::TEXT_COLOR, env.get(ON_RED_KEY));
                  env.set(
                    Key::<druid::Color>::new("button.border"),
                    env.get(ON_RED_KEY),
                  );
                })
                .fix_height(42.0)
                .padding((0.0, 2.0))
                .on_click(|ctx, _, _| ctx.submit_command(Popup::DISMISS)),
            ),
          ),
      )
  }
}

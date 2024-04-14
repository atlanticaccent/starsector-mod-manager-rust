use chrono::{DateTime, Local};
use druid::{
  im::Vector,
  widget::{Flex, Label},
  Data, Key, Widget, WidgetExt,
};
use druid_widget_nursery::wrap::Wrap;
use tap::Pipe;

use crate::{
  app::{
    mod_entry::ModEntry,
    util::{h2_fixed, LabelExt as _, WidgetExtEx as _, BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
    App,
  },
  widgets::card::Card,
};

use super::Popup;

#[derive(Clone, Data)]
pub struct Duplicate(Vector<ModEntry>);

impl Duplicate {
  pub fn new(duplicates: Vector<ModEntry>) -> Self {
    Self(duplicates)
  }

  pub fn view(&self) -> impl Widget<App> {
    let duplicates = self.0.clone();
    Card::builder()
      .with_insets(Card::CARD_INSET)
      .with_background(druid::theme::BACKGROUND_LIGHT)
      .build(
        Flex::column()
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .with_flex_child(
            h2_fixed(&format!(
              r#"Multiple mods with ID {} installed."#,
              &duplicates.front().unwrap().id
            )),
            druid::widget::FlexParams::new(1., druid::widget::CrossAxisAlignment::Center),
          )
          .pipe(|column| {
            let mut column = column;

            for dupe in &duplicates {
              let meta = std::fs::metadata(&dupe.path);
              column.add_child(
                Wrap::new()
                  .direction(druid::widget::Axis::Horizontal)
                  .with_child(Label::wrapped(format!("Version: {}", dupe.version)))
                  .with_child(Label::wrapped(format!(
                    "Path: {}",
                    dupe.path.to_string_lossy()
                  )))
                  .with_child(Label::wrapped(format!(
                    "Last modified: {}",
                    if let Ok(Ok(time)) = meta.as_ref().map(|meta| meta.modified()) {
                      DateTime::<Local>::from(time).format("%F:%R").to_string()
                    } else {
                      "Failed to retrieve last modified".to_string()
                    }
                  )))
                  .with_child(Label::wrapped(format!(
                    "Created at: {}",
                    meta.and_then(|meta| meta.created()).map_or_else(
                      |_| "Failed to retrieve creation time".to_string(),
                      |time| { DateTime::<Local>::from(time).format("%F:%R").to_string() }
                    )
                  )))
                  .with_child(keep_button(dupe.clone(), duplicates.clone())),
              )
            }

            column
          })
          .with_child(
            Card::builder()
              .with_insets((0.0, 8.0))
              .with_corner_radius(6.0)
              .with_shadow_length(2.0)
              .with_shadow_increase(2.0)
              .with_border(2.0, Key::new("button.border"))
              .hoverable(|| {
                Flex::row()
                  .with_child(Label::new("Ignore").padding((10.0, 0.0)))
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
      )
  }
}

fn keep_button(keep: ModEntry, duplicates: Vector<ModEntry>) -> impl Widget<App> {
  Card::builder()
    .with_insets((0.0, 8.0))
    .with_corner_radius(6.0)
    .with_shadow_length(2.0)
    .with_shadow_increase(2.0)
    .with_border(2.0, Key::new("button.border"))
    .hoverable(|| {
      Flex::row()
        .with_child(Label::new("Keep").padding((10.0, 0.0)))
        .align_vertical_centre()
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
    .on_click(move |ctx, data: &mut App, _| {})
}

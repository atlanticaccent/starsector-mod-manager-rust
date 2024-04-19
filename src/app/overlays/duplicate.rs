use chrono::{DateTime, Local};
use druid::{
  im::Vector,
  widget::{Flex, Label},
  Data, Key, Widget, WidgetExt,
};
use druid_widget_nursery::table::{FlexTable, TableColumnWidth, TableRow};
use rand::Rng;
use tap::Pipe;

use crate::{
  app::{
    mod_entry::ModEntry,
    mod_list::ModList,
    util::{h2_fixed, Tap, WidgetExtEx as _, BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
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
          .with_child(
            h2_fixed(&format!(
              r#"Multiple mods with ID "{}" installed."#,
              &duplicates.front().unwrap().id
            ))
            .align_horizontal_centre(),
          )
          .pipe(|column| {
            let mut column = column;

            let mut table = FlexTable::new()
              .with_column_width(TableColumnWidth::Flex(1.0))
              .with_column_width(TableColumnWidth::Intrinsic);
            for (idx, dupe) in duplicates.iter().enumerate() {
              table.add_row(
                TableRow::new()
                  .with_child(dupe_row(dupe))
                  .with_child(keep_button(
                    dupe.clone(),
                    duplicates.clone().tap(|v| v.remove(idx)),
                  )),
              )
            }

            column.add_child(table);

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
              .on_click({
                let insert = duplicates[rand::thread_rng().gen_range(0..duplicates.len())].clone();
                move |ctx, _, _| {
                  ctx.submit_command(Popup::DISMISS);
                  ctx.submit_command(ModList::INSERT_MOD.with(insert.clone()))
                }
              })
              .align_right(),
          )
          .scroll()
          .vertical(),
      )
  }
}

fn dupe_row(dupe: &ModEntry) -> impl Widget<App> {
  let meta = std::fs::metadata(&dupe.path);
  FlexTable::new()
    .with_column_width((TableColumnWidth::Intrinsic, TableColumnWidth::Flex(0.1)))
    .with_column_width(TableColumnWidth::Flex(9.9))
    .with_row(
      TableRow::new()
        .with_child(Label::new("Version:"))
        .with_child(Label::new(dupe.version.to_string())),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("Path:"))
        .with_child(Label::new(dupe.path.to_string_lossy().as_ref())),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("Last modified:"))
        .with_child(Label::new(
          if let Ok(Ok(time)) = meta.as_ref().map(|meta| meta.modified()) {
            DateTime::<Local>::from(time).format("%F:%R").to_string()
          } else {
            "Failed to retrieve last modified".to_owned()
          },
        )),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("Created at:"))
        .with_child(Label::new(
          meta.and_then(|meta| meta.created()).map_or_else(
            |_| "Failed to retrieve creation time".to_owned(),
            |time| DateTime::<Local>::from(time).format("%F:%R").to_string(),
          ),
        )),
    )
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
    .on_click(move |ctx, _, _| {
      let duplicates = duplicates.clone();
      tokio::task::spawn_blocking(move || {
        for dupe in duplicates {
          let _ = remove_dir_all::remove_dir_all(dupe.path);
        }
      });
      ctx.submit_command(Popup::DISMISS);
      ctx.submit_command(ModList::INSERT_MOD.with(keep.clone()));
    })
}

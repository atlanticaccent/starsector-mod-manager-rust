use std::{path::Path, rc::Rc};

use chrono::{DateTime, Local};
use common::{
  labels::{h2, LabelExt},
  lenses::{Convert, LensExtExt},
  widget_ext::WidgetExtEx,
  widgets::card::Card,
  ShadeColor,
};
use derive_more::derive::{From, Into};
use druid::{
  widget::{Checkbox, Flex, Label, Maybe},
  Data, Key, Lens, LensExt, Selector, Widget, WidgetExt,
};
use druid_patch::table::{RowData, TableData};
use druid_widget_nursery::table::{FlexTable, TableColumnWidth, TableRow};
use macros::OptionSpec;
use ref_cast::RefCast;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use uuid::Uuid;

use super::Popup;
use crate::{
  app::{
    controllers::GLOBAL_ASYNC_CONTROLLER,
    mod_entry::{ModEntry as RawModEntry, ViewModEntry as ModEntry},
    mod_list::ModList,
    settings::Settings,
    App,
  },
  theme::{BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
};

const KEEP_ENTRY: Selector<Uuid> = Selector::new("app.popup.duplicate.keep");

#[derive(Clone, Data)]
pub struct Duplicate(String);

#[OptionSpec]
#[derive(Debug, Clone, Data, Lens)]
struct DuplicateFocus {
  entry: Option<Rc<ModEntry>>,
  show_duplicates: bool,
  mods_folder: Rc<Path>,
}

type TupleDuplicateFocus = (Option<Rc<ModEntry>>, bool, Rc<Path>);

struct DuplicateLens;

impl Lens<TupleDuplicateFocus, DuplicateFocus> for DuplicateLens {
  fn with<V, F: FnOnce(&DuplicateFocus) -> V>(&self, data: &TupleDuplicateFocus, f: F) -> V {
    let (entry, show_duplicates, mods_folder) = data.clone();
    let focus = DuplicateFocus {
      entry,
      show_duplicates,
      mods_folder,
    };
    f(&focus)
  }

  fn with_mut<V, F: FnOnce(&mut DuplicateFocus) -> V>(
    &self,
    data: &mut TupleDuplicateFocus,
    f: F,
  ) -> V {
    let mut focus = DuplicateFocus {
      entry: std::mem::take(&mut data.0),
      show_duplicates: std::mem::take(&mut data.1),
      mods_folder: std::mem::replace(&mut data.2, Path::new("").into()),
    };

    let res = f(&mut focus);

    data.0 = focus.entry;
    data.1 = focus.show_duplicates;
    data.2 = focus.mods_folder;

    res
  }
}

impl Duplicate {
  pub fn new(duplicates: String) -> Self {
    Self(duplicates)
  }

  pub fn view(&self) -> impl Widget<App> {
    let dupe_id = self.0.clone();
    Maybe::or_empty(Self::view_inner(&self.0)).lens(
      (
        App::mod_list.then(ModList::mods).map(
          {
            let dupe_id = dupe_id.clone();
            move |mods| mods.get(&dupe_id).cloned()
          },
          move |mods, entry| {
            if let Some(entry) = entry {
              mods.entry(dupe_id.clone()).and_modify(|old| *old = entry);
            }
          },
        ),
        App::settings.then(Settings::show_duplicate_warnings),
        App::settings
          .then(Settings::install_dir)
          .compute(|path| Rc::from(path.as_ref().map(|p| p.as_path()).unwrap_or(Path::new("")))),
      )
        .then(DuplicateLens.then(DuplicateFocus::invert_on_entry)),
    )
  }

  fn view_inner(id: &str) -> impl Fn() -> impl Widget<EntryInverseDuplicateFocus> {
    let id = id.to_owned();
    move || {
      Card::builder()
        .with_insets(Card::CARD_INSET)
        .with_background(druid::theme::BACKGROUND_LIGHT)
        .build(
          Flex::column()
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            .with_child(
              h2()
                .halign_centre()
                .lens(EntryInverseDuplicateFocus::entry.compute(|entry| {
                  format!(r#"Multiple mods with ID "{}" installed."#, entry.mod_id)
                })),
            )
            .with_child({
              let table = FlexTable::new()
                .with_column_width(TableColumnWidth::Flex(1.0))
                .with_column_width(TableColumnWidth::Intrinsic)
                .on_notification(KEEP_ENTRY, handle_keep_notif);

              table.lens(EntryInverseDuplicateFocus::entry.then(Convert::new().in_rc()))
            })
            .with_child(
              Flex::row()
                .main_axis_alignment(druid::widget::MainAxisAlignment::End)
                .with_child(Checkbox::from_label(Label::wrapped(
                  "Show warnings when duplicates of a mod are installed",
                )))
                .with_child(
                  Card::builder()
                    .with_insets((0.0, 8.0))
                    .with_corner_radius(6.0)
                    .with_shadow_length(2.0)
                    .with_shadow_increase(2.0)
                    .with_border(2.0, Key::new("button.border"))
                    .hoverable(|_| {
                      Flex::row()
                        .with_child(Label::new("Ignore All").padding((10.0, 0.0)))
                        .valign_centre()
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
                      let id = id.clone();
                      move |ctx, _, _| {
                        let id = id.clone();
                        ctx.submit_command(Popup::dismiss_matching(
                          move |popup| matches!(popup, Popup::Duplicate(dupe) if dupe.0 == id),
                        ));
                      }
                    }),
                )
                .with_child(
                  Card::builder()
                    .with_insets((0.0, 8.0))
                    .with_corner_radius(6.0)
                    .with_shadow_length(2.0)
                    .with_shadow_increase(2.0)
                    .with_border(2.0, druid::Color::WHITE.darker())
                    .with_background(druid::Color::BLACK.lighter().lighter())
                    .hoverable(|_| {
                      Flex::row()
                        .with_child(Label::new("Ignore").padding((10.0, 0.0)))
                        .valign_centre()
                    })
                    .env_scope(|env, _| {
                      env.set(druid::theme::TEXT_COLOR, druid::Color::WHITE.darker());
                    })
                    .fix_height(42.0)
                    .padding((0.0, 2.0))
                    .on_click({
                      let id = id.clone();
                      move |ctx, show_duplicate_warnings, _| {
                        if *show_duplicate_warnings {
                          ctx.submit_command(Popup::DISMISS);
                        } else {
                          let id = id.clone();
                          ctx.submit_command(Popup::dismiss_matching(
                            move |popup| matches!(popup, Popup::Duplicate(dupe) if dupe.0 == id),
                          ));
                        }
                      }
                    }),
                )
                .align_right()
                .lens(EntryInverseDuplicateFocus::show_duplicates),
            )
            .scroll()
            .vertical(),
        )
    }
  }
}

fn dupe_row() -> impl Widget<ModEntry> {
  FlexTable::new()
    .with_column_width((TableColumnWidth::Intrinsic, TableColumnWidth::Flex(0.1)))
    .with_column_width(TableColumnWidth::Flex(9.9))
    .with_row(
      TableRow::new()
        .with_child(Label::new("Version:"))
        .with_child(Label::dynamic(|entry: &ModEntry, _| {
          entry.version.to_string()
        })),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("Path:"))
        .with_child(Label::dynamic(|entry: &ModEntry, _| {
          entry.path.to_string_lossy().to_string()
        })),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("Last modified:"))
        .with_child(Label::dynamic(|entry: &ModEntry, _| {
          if let Ok(time) = entry.path.metadata().and_then(|entry| entry.modified()) {
            DateTime::<Local>::from(time).format("%F:%R").to_string()
          } else {
            "Failed to retrieve last modified".to_owned()
          }
        })),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("Created at:"))
        .with_child(Label::dynamic(|entry: &ModEntry, _| {
          if let Ok(time) = entry.path.metadata().and_then(|meta| meta.created()) {
            DateTime::<Local>::from(time).format("%F:%R").to_string()
          } else {
            "Failed to retrieve creation time".to_owned()
          }
        })),
    )
}

fn keep_button() -> impl Widget<ModEntry> {
  Card::builder()
    .with_insets((0.0, 8.0))
    .with_corner_radius(6.0)
    .with_shadow_length(2.0)
    .with_shadow_increase(2.0)
    .with_border(2.0, Key::new("button.border"))
    .hoverable(|_| {
      Flex::row()
        .with_child(Label::new("Keep").padding((10.0, 0.0)))
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
    .on_click(move |ctx, data: &mut ModEntry, _| {
      ctx.submit_notification(KEEP_ENTRY.with(data.internal_id));
    })
}

fn handle_keep_notif(
  ctx: &mut druid::EventCtx,
  internal_id: &Uuid,
  EntryWithDuplicates(entry): &mut EntryWithDuplicates,
) {
  let dupe_paths: Vec<RawModEntry> = if entry.internal_id == *internal_id {
    entry.duplicates.iter().map(|entry| entry.into()).collect()
  } else if let Some(chosen) = entry
    .duplicates
    .iter()
    .find(|dupe| dupe.internal_id == *internal_id)
  {
    let original = std::mem::replace(entry, chosen.clone());
    original
      .iter_with_dupes()
      .filter(|entry| entry.internal_id != *internal_id)
      .map(|entry| entry.into())
      .collect()
  } else {
    return;
  };
  entry.duplicates = Default::default();

  GLOBAL_ASYNC_CONTROLLER
    .get_unchecked()
    .add_task(archive_duplicates(todo!(), dupe_paths), |_, ctx, app, _| {});

  // todo: delete or archive all the other entries
  ctx.submit_command(Popup::DISMISS);
}

async fn archive_duplicates(mod_folder: &Path, dupes: Vec<RawModEntry>) -> anyhow::Result<()> {
  tokio::fs::create_dir_all(mod_folder).await?;

  let futures = dupes.into_iter().map(async |entry| -> anyhow::Result<()> {
    use zip::ZipWriter;

    let backup_name = format!("{}-{}-{}", entry.mod_id, entry.version, entry.internal_id);
    let archive = tokio::fs::File::options()
      .share_mode(0)
      .write(true)
      .truncate(true)
      .create(true)
      .open(mod_folder.join(backup_name).with_extension("zip"))
      .await?;

    let bridged_archive = tokio_util::io::SyncIoBridge::new(archive);

    tokio::task::spawn_blocking(move || {
      let writer = ZipWriter::new(bridged_archive);

    }).await?;

    Ok(())
  });

  Ok(())
}

#[derive(Debug, Clone, Data, RefCast, Lens)]
#[repr(transparent)]
struct DuplicateEntry {
  entry: ModEntry,
}

#[derive(Debug, Clone, Data, Into, From)]
struct EntryWithDuplicates(ModEntry);

#[derive(Debug, PartialEq, Eq, Hash, Clone, EnumIter)]
enum Columns {
  Entry,
  Button,
}

impl RowData for DuplicateEntry {
  type Id = Uuid;
  type Column = Columns;

  fn id(&self) -> Self::Id {
    self.entry.internal_id
  }

  fn cell(&self, column: &Self::Column) -> Box<dyn Widget<Self>> {
    match column {
      Columns::Entry => dupe_row().lens(DuplicateEntry::entry).boxed(),
      Columns::Button => keep_button().lens(DuplicateEntry::entry).boxed(),
    }
  }
}

impl TableData for EntryWithDuplicates {
  type Row = DuplicateEntry;
  type Column = Columns;

  fn keys(&self) -> impl Iterator<Item = Uuid> {
    self.0.iter_with_dupes().map(|entry| entry.internal_id)
  }

  fn columns(&self) -> impl Iterator<Item = Self::Column> {
    Columns::iter()
  }

  fn with_mut(&mut self, id: Uuid, mutate: impl FnOnce(&mut DuplicateEntry)) {
    let mut mutate = Some(mutate);
    self.0.iter_with_dupes_mut(|entry| {
      if entry.internal_id == id {
        mutate.take().unwrap()(DuplicateEntry::ref_cast_mut(entry))
      }
    });
  }

  fn index(&self, id: Uuid) -> &DuplicateEntry {
    self
      .0
      .iter_with_dupes()
      .find_map(|entry| {
        if entry.internal_id == id {
          Some(DuplicateEntry::ref_cast(entry))
        } else {
          None
        }
      })
      .unwrap()
  }
}

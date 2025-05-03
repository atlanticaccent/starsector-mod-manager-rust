use std::{
  path::{Path, PathBuf},
  rc::Rc,
};

use chrono::{DateTime, Local};
use common::{
  labels::{h2, LabelExt},
  lenses::LensExtExt,
  row,
  widget_ext::WidgetExtEx,
  widgets::card::Card,
  ShadeColor,
};
use druid::{
  theme::BACKGROUND_DARK,
  widget::{Checkbox, Either, Flex, Label, Maybe, Painter},
  Data, Key, Lens, LensExt, Selector, Widget, WidgetExt as _,
};
use druid_patch::{
  switch::Switch,
  table::{FixedFlexTable, TableCellVerticalAlignment, TableColumnWidth, TableRow},
};
use frunk::{Generic, LabelledGeneric};
use futures_util::{StreamExt, TryStreamExt};
use itertools::Itertools;
use macros::OptionSpec;
use remove_dir_all::RemoveDir;
use uuid::Uuid;
use zip_extensions::ZipWriterExtensions;

use crate::{
  app::{
    controllers::GLOBAL_ASYNC_CONTROLLER,
    mod_entry::{DuplicateGuard, ModEntry as RawModEntry, ViewModEntry as ModEntry, ViewState},
    mod_list::ModList,
    settings::Settings,
    util::Tap,
    App, Popup,
  },
  bang,
  theme::{BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
};

const KEEP_ENTRY: Selector<Uuid> = Selector::new("app.popup.duplicate.keep");

#[derive(Clone, Data)]
pub struct Duplicate(String);

#[OptionSpec()]
#[derive(Debug, Clone, Data, Lens, Generic, LabelledGeneric)]
struct DuplicateFocus {
  entry: Option<Rc<ModEntry>>,
  show_duplicates: bool,
  archive_duplicates: bool,
  mods_folder: Rc<Path>,
  dupe_warnings: usize,
}

type TupleDuplicateFocus = (Option<Rc<ModEntry>>, bool, bool, Rc<Path>, usize);

struct DuplicateLens;

impl Lens<TupleDuplicateFocus, DuplicateFocus> for DuplicateLens {
  fn with<V, F: FnOnce(&DuplicateFocus) -> V>(&self, data: &TupleDuplicateFocus, f: F) -> V {
    f(&frunk::convert_from(data.clone()))
  }

  fn with_mut<V, F: FnOnce(&mut DuplicateFocus) -> V>(
    &self,
    data: &mut TupleDuplicateFocus,
    f: F,
  ) -> V {
    let mut focus: DuplicateFocus = frunk::convert_from(data.clone());

    let res = f(&mut focus);

    *data = frunk::convert_from(focus);

    res
  }
}

impl Duplicate {
  pub fn new(duplicates: String) -> Self {
    Self(duplicates)
  }

  pub fn view(&self) -> impl Widget<App> {
    let dupe_id = self.0.clone();
    Maybe::or_empty({
      let builder = Self::view_inner(&self.0);
      move || row![.flex, 1; builder(), 3; .flex, 1].expand_width()
    })
    .on_added(|_, ctx, data, _| {
      if data
        .as_ref()
        .map(|data| !data.show_duplicates)
        .unwrap_or_default()
      {
        ctx.submit_command(Popup::DISMISS);
      }
    })
    .lens(
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
        App::settings.then(Settings::archive_duplicates),
        App::settings
          .then(Settings::install_dir)
          .compute(|path| Rc::from(path.as_ref().map(|p| p.as_path()).unwrap_or(Path::new("")))),
        App::popups.compute(|popups| {
          popups
            .iter()
            .filter(|popup| matches!(popup, Popup::Duplicate(_)))
            .count()
        }),
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
              let table = FixedFlexTable::new()
                .with_column_width(TableColumnWidth::Flex(1.0))
                .with_column_width(TableColumnWidth::Intrinsic)
                .default_vertical_alignment(TableCellVerticalAlignment::Fill)
                .row_background(Painter::new(move |ctx, _, env| {
                  use druid::RenderContext;

                  let x_pad = env.get(druid::theme::WIDGET_PADDING_HORIZONTAL);
                  let y_pad = env.get(druid::theme::WIDGET_PADDING_VERTICAL);

                  let rect = ctx
                    .region()
                    .bounding_box()
                    .inset((-x_pad, -y_pad))
                    .to_rounded_rect(8.0);
                  ctx.fill(rect, &env.get(BACKGROUND_DARK));
                }));

              table
                .on_added(|table, ctx, data: &Rc<ModEntry>, env| {
                  let x_pad = env.get(druid::theme::WIDGET_PADDING_HORIZONTAL) * 2.0;
                  let y_pad = env.get(druid::theme::WIDGET_PADDING_VERTICAL) * 2.0;

                  for entry in data
                    .guarded_dupe_iter()
                    .sorted_by_cached_key(|entry| entry.path.to_string_lossy().into_owned())
                  {
                    table.add_row(
                      TableRow::new()
                        .with_child(dupe_row().padding((x_pad, y_pad)).constant(entry.clone()))
                        .with_child(keep_button().padding((x_pad, y_pad)).constant(entry)),
                    );
                  }

                  ctx.children_changed();
                  ctx.request_layout();
                  ctx.request_paint();
                })
                .lens(EntryInverseDuplicateFocus::entry)
                .on_notification(KEEP_ENTRY, handle_keep_notif)
            })
            .with_default_spacer()
            .with_child(
              row![
                Checkbox::new("");
                Label::wrapped("Don't warn me when duplicates of a mod are installed")
                .on_click(|_, data: &mut bool, _| {
                  *data = !*data
                })
              ]
              .cross_axis_alignment(druid::widget::CrossAxisAlignment::Center)
              .env_scope(|env, _| {
                let height = env.get(druid::theme::BASIC_WIDGET_HEIGHT);
                env.set(druid::theme::BASIC_WIDGET_HEIGHT, height * 1.2);
              })
              .halign_centre()
              .lens(EntryInverseDuplicateFocus::show_duplicates),
            )
            .with_child(
              Flex::row()
                .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
                .with_child(
                  Flex::row()
                    .with_child(Label::wrapped("Delete"))
                    .with_child(
                      Switch::new()
                        .textless()
                        .width(40.)
                        .off_color(druid::theme::PRIMARY_DARK)
                        .off_color_stop(druid::theme::PRIMARY_LIGHT)
                        .padding((8., 0.))
                        .lens(EntryInverseDuplicateFocus::archive_duplicates),
                    )
                    .with_child(Label::wrapped("Backup")),
                )
                .with_child(
                  Flex::row()
                    .main_axis_alignment(druid::widget::MainAxisAlignment::End)
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
                        .on_click(|ctx, _, _| {
                          ctx.submit_command(Popup::dismiss_matching(move |popup| {
                            matches!(popup, Popup::Duplicate(_))
                          }));
                        })
                        .empty_if(|count, _| *count <= 1)
                        .lens(EntryInverseDuplicateFocus::dupe_warnings),
                    )
                    .with_child({
                      let builder =
                        |front: druid::KeyOrValue<druid::Color>,
                         back: druid::KeyOrValue<druid::Color>| {
                          let back: druid::widget::BackgroundBrush<()> = match back {
                            druid::KeyOrValue::Concrete(color) => color.into(),
                            druid::KeyOrValue::Key(key) => key.into(),
                          };
                          Card::builder()
                            .with_insets((0.0, 8.0))
                            .with_corner_radius(6.0)
                            .with_shadow_length(2.0)
                            .with_shadow_increase(2.0)
                            .with_border(2.0, front.clone())
                            .with_background(back)
                            .hoverable(move |_| {
                              Flex::row()
                                .with_child(
                                  Label::new("Ignore")
                                    .with_text_color(front.clone())
                                    .padding((10.0, 0.0)),
                                )
                                .valign_centre()
                            })
                        };

                      Either::new(
                        |focus: &EntryInverseDuplicateFocus, _| focus.dupe_warnings <= 1,
                        builder(ON_RED_KEY.into(), RED_KEY.into()),
                        builder(
                          druid::Color::WHITE.darker().into(),
                          druid::Color::BLACK.lighter().lighter().into(),
                        ),
                      )
                      .fix_height(42.0)
                      .padding((0.0, 2.0))
                      .on_click({
                        let id = id.clone();
                        move |ctx, focus: &mut EntryInverseDuplicateFocus, _| {
                          if focus.show_duplicates {
                            ctx.submit_command(Popup::DISMISS);
                          } else {
                            let id = id.clone();
                            ctx.submit_command(Popup::dismiss_matching(
                              move |popup| matches!(popup, Popup::Duplicate(dupe) if dupe.0 == id),
                            ));
                          }
                        }
                      })
                    })
                    .expand_width(),
                )
                .expand_width(),
            )
            .scroll()
            .vertical(),
        )
    }
  }
}

fn dupe_row() -> impl Widget<DuplicateGuard<ViewState>> {
  FixedFlexTable::new()
    .with_column_width((TableColumnWidth::Intrinsic, TableColumnWidth::Flex(0.1)))
    .with_column_width(TableColumnWidth::Flex(9.9))
    .with_row(
      TableRow::new()
        .with_child(Label::new("Version:"))
        .with_child(Label::dynamic(|entry: &DuplicateGuard<ViewState>, _| {
          entry.version.to_string()
        })),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("Path:"))
        .with_child(Label::dynamic(|entry: &DuplicateGuard<ViewState>, _| {
          entry.path.to_string_lossy().to_string()
        })),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("Last modified:"))
        .with_child(Label::dynamic(|entry: &DuplicateGuard<ViewState>, _| {
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
        .with_child(Label::dynamic(|entry: &DuplicateGuard<ViewState>, _| {
          if let Ok(time) = entry.path.metadata().and_then(|meta| meta.created()) {
            DateTime::<Local>::from(time).format("%F:%R").to_string()
          } else {
            "Failed to retrieve creation time".to_owned()
          }
        })),
    )
}

fn keep_button() -> impl Widget<DuplicateGuard<ViewState>> {
  Card::builder()
    .with_insets((0.0, 8.0))
    .with_corner_radius(6.0)
    .with_shadow_length(2.0)
    .with_shadow_increase(6.0)
    .hoverable(|_| {
      Flex::row()
        .with_child(
          Label::new("Keep")
            .with_text_size(16.0)
            .with_font(druid::theme::UI_FONT_BOLD)
            .padding((32.0, 0.0)),
        )
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
    .fix_height(32.0)
    .padding((0.0, 2.0))
    .on_click(move |ctx, data: &mut DuplicateGuard<ViewState>, _| {
      ctx.submit_notification(KEEP_ENTRY.with(data.internal_id));
    })
}

fn handle_keep_notif(
  ctx: &mut druid::EventCtx,
  internal_id: &Uuid,
  view: &mut EntryInverseDuplicateFocus,
) {
  let entry = Rc::make_mut(&mut view.entry);

  let dupes: Vec<RawModEntry> = if entry.internal_id == *internal_id {
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

  let mods_folder = view.mods_folder.to_path_buf();
  let backup = view.archive_duplicates;
  GLOBAL_ASYNC_CONTROLLER.get_unchecked().add_task(
    async move { archive_duplicates(mods_folder, dupes, backup).await },
    |res, _ctx, _, _| {
      if let Err(err) = res {
        bang!(err)
      }
    },
  );

  if view.show_duplicates {
    ctx.submit_command(Popup::DISMISS);
  } else {
    ctx.submit_command(Popup::dismiss_matching(|popup| {
      matches!(popup, Popup::Duplicate(_))
    }));
  }
}

async fn archive_duplicates(
  mod_folder: PathBuf,
  dupes: Vec<RawModEntry>,
  backup: bool,
) -> anyhow::Result<()> {
  let backup_folder = if backup {
    let backup_folder = mod_folder.tap(|folder| {
      folder.push("mods");
      folder.push(".moss_backups");
    });

    tokio::fs::create_dir_all(&backup_folder).await?;
    Some(backup_folder)
  } else {
    None
  };

  let futures = dupes.into_iter().map(async |entry| -> anyhow::Result<()> {
    use zip::ZipWriter;

    let bridged_archive = if let Some(backup_folder) = backup_folder.as_ref() {
      let backup_name = format!("{}-{}-{}", entry.mod_id, entry.version, entry.internal_id);
      let archive = match tokio::fs::File::options()
        .write(true)
        .create_new(true)
        .open(backup_folder.join(backup_name).with_extension("zip"))
        .await
      {
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => None,
        err => Some(err?),
      };

      archive.map(tokio_util::io::SyncIoBridge::new)
    } else {
      None
    };

    tokio::task::spawn_blocking(move || {
      if let Some(bridged_archive) = bridged_archive {
        let writer = ZipWriter::new(bridged_archive);

        writer.create_from_directory(&entry.path)?;
      }

      let mut opts = std::fs::File::options();

      #[cfg(windows)]
      let opts = std::os::windows::fs::OpenOptionsExt::custom_flags(&mut opts, 0x02000000);

      opts
        .read(true)
        .write(true)
        .open(&entry.path)?
        .remove_dir_contents(Some(&entry.path))?;

      std::fs::remove_dir(entry.path)
    })
    .await??;

    Ok(())
  });

  futures_util::stream::iter(futures)
    .buffer_unordered(
      std::thread::available_parallelism().map_or_else(|_| 4, std::num::NonZero::get),
    )
    .try_collect::<()>()
    .await?;

  Ok(())
}

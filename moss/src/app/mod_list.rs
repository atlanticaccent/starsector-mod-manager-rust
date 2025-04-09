use std::{
  hash::Hash,
  num::NonZero,
  ops::{Deref, Index},
  path::{Path, PathBuf},
  rc::Rc,
  sync::Arc,
};

use ahash::{AHashSet, HashSet};
use anyhow::Context;
use comemo::memoize;
use common::{
  controllers::ExtensibleController,
  fast_im_map::FastImMap,
  lenses::{Compute, LensExtExt},
  widget_ext::WidgetExtEx as _,
  widgets::card::Card,
  ExtEventSinkExt,
};
use druid::{
  im::Vector,
  lens, theme,
  widget::{Flex, Label, Painter},
  Data, EventCtx, ExtEventSink, Lens, LensExt, Rect, RenderContext, Selector, SingleUse, Widget,
  WidgetExt,
};
use druid_patch::table::{
  ComplexTableColumnWidth, FixedFlexTable, FlexTable, RowData, TableCellVerticalAlignment,
  TableColumnWidth, TableData, TableRow,
};
use druid_widget_nursery::{
  Stack, StackChildParams, StackChildPosition, WidgetExt as WidgetExtNursery,
};
use futures_util::{StreamExt, TryStreamExt};
use installer::HybridPath;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumCount, EnumIter};
use sublime_fuzzy::best_match;
use web_client::WebClient;

use super::{
  mod_entry::{
    GameVersion, ModEntry as RawModEntry, ModMetadata, UpdateStatus, ViewModEntry as ModEntry,
  },
  util::SaveError,
  App,
};
use crate::app::mod_list::actions::{
  action_button::ActionsButton, action_options::ActionsOptions, ActionsState,
};

mod actions;
pub mod filters;
pub mod headings;
pub mod install;
mod refresh;
pub mod search;

use self::{
  filters::{
    filter_button::FilterButton, filter_options::FilterOptions, FilterState, FILTER_POSITION,
  },
  headings::{Header, Heading},
  install::{install_button::InstallButton, install_options::InstallOptions, InstallState},
  refresh::Refresh,
  search::Search,
};

const CONTROL_WIDTH: f64 = 185.0;

#[derive(Clone, Data, Lens)]
pub struct ModList {
  pub mods: FastImMap<String, Rc<ModEntry>>,
  pub header: Header,
  pub search_text: String,
  pub starsector_version: Option<GameVersion>,
  install_state: InstallState,
  actions_state: ActionsState,
  pub filter_state: FilterState,
  pub install_dir_available: bool,
  pub refreshing: bool,
}

type RawModMap = FastImMap<String, RawModEntry>;
pub type ModMap = FastImMap<String, Rc<ModEntry>>;

impl ModList {
  pub const AUTO_UPDATE: Selector<ModEntry> = Selector::new("mod_list.install.auto_update");
  pub const DUPLICATE: Selector<(ModEntry, ModEntry)> =
    Selector::new("mod_list.submit_entry.duplicate");
  pub const FILTER_RESET: Selector = Selector::new("mod_list.filter.reset");
  pub const FILTER_UPDATE: Selector<(Filters, bool)> = Selector::new("mod_list.filter.update");
  pub const INSERT_MOD: Selector<RawModEntry> = Selector::new("mod_list.mods.insert");
  pub const OVERWRITE: Selector<SingleUse<(PathBuf, HybridPath, RawModEntry)>> =
    Selector::new("mod_list.install.overwrite");
  pub const REBUILD: Selector = Selector::new("mod_list.table.rebuild");
  pub const REBUILD_NEXT_PASS: Selector = Selector::new("mod_list.table.rebuild_next_pass");
  pub const SEARCH_UPDATE: Selector<bool> = Selector::new("mod_list.filter.search.update");
  pub const UPDATE_COLUMN_WIDTH: Selector<(usize, f64)> =
    Selector::new("mod_list.column.update_width");
  const UPDATE_TABLE_SORT: Selector = Selector::new("mod_list.table.update_sorting");
  // const UPDATE_VERSION_CHECKER: Selector<
  //   HashMap<(String, VersionComplex), Option<ModVersionMeta>>,
  // > = Selector::new("mod_list.mods.update_version_checker");

  pub fn new(headings: Vector<Heading>) -> Self {
    Self {
      mods: FastImMap::new(),
      header: Header::new(headings),
      search_text: String::new(),
      starsector_version: None,
      install_state: InstallState::default(),
      actions_state: ActionsState::default(),
      filter_state: FilterState::default(),
      install_dir_available: false,
      refreshing: false,
    }
  }

  pub fn replace_mods(&mut self, mods: FastImMap<String, RawModEntry>) {
    *self.mods = mods
      .inner()
      .into_iter()
      .map(|(id, entry)| (id, Rc::new(ModEntry::from(entry))))
      .collect();
  }

  pub fn view() -> impl Widget<Self> {
    Stack::new()
      .with_child(
        Flex::column()
          .with_child(
            FixedFlexTable::new()
              .column_widths(&[
                TableColumnWidth::Fraction(0.35).into(),
                TableColumnWidth::Fraction(0.15).into(),
                TableColumnWidth::Fraction(0.35).into(),
                TableColumnWidth::Fraction(0.15).into(),
              ])
              .with_row(
                TableRow::new()
                  .vertical_alignment(TableCellVerticalAlignment::Middle)
                  .with_child(fixed_bold_text("Installed:").align_left())
                  .with_child(lensed_bold_text().lens(lens!((usize, usize), 0)).center())
                  .with_child(fixed_bold_text("Enabled:").align_left())
                  .with_child(lensed_bold_text().lens(lens!((usize, usize), 1)).center()),
              )
              .fix_width(CONTROL_WIDTH * 1.2)
              .lens(Compute::new(|mod_list: &ModList| {
                (
                  mod_list.mods.len(),
                  mod_list.mods.values().filter(|it| it.enabled).count(),
                )
              }))
              .padding((7.0, 0.0))
              .align_left(),
          )
          .with_child(
            Flex::row()
              .with_child(Refresh::view().padding((0.0, 5.0)))
              .with_child(
                InstallButton::view()
                  .lens(Self::install_state)
                  .padding((0.0, 5.0))
                  .disabled_if(|data, _| !data.install_dir_available),
              )
              .with_child(
                ActionsButton::view()
                  .lens(Self::actions_state)
                  .padding((0.0, 5.0))
                  .disabled_if(|data: &ModList, _| data.mods.is_empty()),
              )
              .with_flex_spacer(1.0)
              .with_child(FilterButton::view().lens(Self::filter_state))
              .with_child(
                Search::view()
                  .lens(Self::search_text)
                  .on_change(|ctx, old, data, _| {
                    if !old.same(data) {
                      if data.search_text.is_empty() {
                        data.header.sort_by = (Heading::Name, true);
                      } else {
                        data.header.sort_by = (Heading::Score, true);
                      };
                      ctx.submit_command(Self::UPDATE_TABLE_SORT);
                    }
                  }),
              )
              .expand_width(),
          )
          .with_flex_child(
            Card::builder()
              .with_insets((0.0, 14.0))
              .with_corner_radius(4.0)
              .with_shadow_length(6.0)
              .build(
                Flex::column()
                  .with_child(headings::Header::view().lens(ModList::header))
                  .with_flex_child(
                    FlexTable::default()
                      .row_background(Painter::new(move |ctx, _, env| {
                        let rect = ctx.size().to_rect();

                        if env.try_get(FlexTable::<ModList>::ROW_IDX).unwrap_or(0) % 2 == 0 {
                          ctx.fill(rect, &env.get(theme::BACKGROUND_DARK));
                        } else {
                          ctx.fill(rect, &env.get(theme::BACKGROUND_LIGHT));
                        }
                      }))
                      .with_column_width(TableColumnWidth::Fixed(Header::ENABLED_WIDTH))
                      .default_vertical_alignment(TableCellVerticalAlignment::Fill)
                      .column_border(theme::BORDER_DARK, 1.0)
                      .clip_aware(true)
                      .controller(
                        ExtensibleController::new()
                          .on_command(Self::UPDATE_COLUMN_WIDTH, Self::column_resized)
                          .on_command(Self::UPDATE_TABLE_SORT, |_, ctx, payload, data| {
                            Self::update_sorting(ctx, payload, data)
                          })
                          .on_command(ModMetadata::SUBMIT_MOD_METADATA, Self::metadata_submitted)
                          .on_command(Self::FILTER_UPDATE, Self::on_filter_change)
                          .on_command(Self::FILTER_RESET, Self::on_filter_reset)
                          .on_command(Self::REBUILD, |table, ctx, (), _| {
                            table.clear();
                            ctx.children_changed();
                            ctx.request_update();
                            ctx.request_layout();
                            ctx.request_paint();

                            true
                          })
                          .on_command(Self::INSERT_MOD, |_, ctx, entry, data| {
                            data
                              .mods
                              .insert(entry.mod_id.clone(), Rc::new(entry.clone().into()));
                            ctx.request_update();
                            true
                          })
                          .on_command(App::ENABLE, |_, ctx, _, _| {
                            ctx.children_changed();
                            true
                          })
                          .on_lifecycle(|table, ctx, event, data, env| {
                            if let druid::LifeCycle::BuildFocusChain = event
                              && !ctx.is_disabled()
                            {
                              table.lifecycle(
                                ctx,
                                &druid::LifeCycle::DisabledChanged(false),
                                data,
                                env,
                              );
                            }
                          }),
                      )
                      .scroll()
                      .vertical()
                      .expand_width(),
                    1.0,
                  )
                  .on_change(|ctx, old, data, _| {
                    if !old.header.same(&data.header) || !old.mods.same(&data.mods) {
                      ctx.request_paint();
                      ctx.request_update();
                      ctx.request_layout();
                    }
                  }),
              ),
            1.0,
          ),
      )
      .with_positioned_child(
        InstallOptions::view()
          .lens(Self::install_state)
          .padding((0.0, 5.0)),
        StackChildPosition::default()
          .top(Some(18.0))
          .left(Some(52.0)),
      )
      .with_positioned_child(
        ActionsOptions::view().padding((0.0, 5.0)),
        StackChildPosition::default()
          .top(Some(18.0))
          .left(Some(52.0 + CONTROL_WIDTH)),
      )
      .with_positioned_child(
        FilterOptions::view().lens(Self::filter_state),
        StackChildParams::dynamic(|data: &ModList, _| &data.filter_state.stack_position)
          .duration(0.0),
      )
      .with_positioned_child(
        FilterOptions::wide_view().lens(Self::filter_state),
        StackChildPosition::default()
          .top(Some(72.0))
          .left(Some(0.0))
          .right(Some(0.0)),
      )
      .fit(true)
      .on_command(FILTER_POSITION, |ctx, point, data| {
        let rect = Rect::from_points(ctx.window_origin(), *point);
        data.filter_state.stack_position.top = Some(rect.height());
        data.filter_state.stack_position.left = Some(rect.width());
      })
      .mask_default()
      .dynamic(|data, _| data.refreshing)
      .with_text_mask("")
      .on_command(App::REFRESH, |_, (), data| {
        data.refreshing = true;
      })
      .on_command(App::REPLACE_MODS, Self::replace_mods_command_handler)
  }

  fn replace_mods_command_handler(
    ctx: &mut EventCtx,
    payload: &SingleUse<FastImMap<String, RawModEntry>>,
    data: &mut ModList,
  ) {
    data.refreshing = false;
    data.replace_mods(payload.take().unwrap());

    Self::update_sorting(ctx, &(), data);
    ctx.request_update();
    ctx.children_changed();
  }

  fn metadata_submitted(
    _table: &mut FlexTable<ModList>,
    _ctx: &mut EventCtx,
    (id, metadata): &(String, ModMetadata),
    data: &mut ModList,
  ) -> bool {
    ModList::mods
      .deref()
      .index(id)
      .then(ModEntry::manager_metadata.in_rc())
      .put(data, metadata.clone());

    false
  }

  fn column_resized(
    table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    payload: &(usize, f64),
    _data: &mut ModList,
  ) -> bool {
    if !table.is_empty() {
      let column_count = table.column_count();
      let widths = table.get_column_widths();
      if widths.len() < column_count {
        widths.resize_with(column_count, || {
          ComplexTableColumnWidth::Simple(TableColumnWidth::Flex(1.0))
        });
      }
      widths[payload.0] = ComplexTableColumnWidth::Simple(TableColumnWidth::Fixed(payload.1 - 1.0));

      ctx.request_update();
      ctx.request_layout();
    }

    false
  }

  fn update_sorting<P>(ctx: &mut EventCtx, _: &P, _: &mut ModList) -> bool {
    ctx.request_update();
    ctx.request_layout();
    ctx.request_paint();
    false
  }

  pub fn on_app_data_change(
    _ctx: &mut EventCtx,
    old: &super::App,
    data: &mut super::App,
    _env: &druid::Env,
  ) {
    if let Some(install_dir) = &data.settings.install_dir {
      let diff = old
        .mod_list
        .mods
        .deref()
        .clone()
        .difference_with(data.mod_list.mods.clone().into(), |left, right| {
          (left.enabled != right.enabled).then_some(right)
        });

      if !diff.is_empty() {
        let enabled: Vec<String> = data
          .mod_list
          .mods
          .values()
          .filter_map(|v| v.enabled.then_some(v.mod_id.clone()))
          .collect();
        if let Err(err) = EnabledMods::from(enabled).save(install_dir) {
          eprintln!("{err:?}");
        };
      }
    }
  }

  #[allow(clippy::trivially_copy_pass_by_ref)]
  fn on_filter_change(
    _table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    payload: &(Filters, bool),
    data: &mut ModList,
  ) -> bool {
    if payload.1 {
      data.filter_state.active_filters.insert(payload.0)
    } else {
      data.filter_state.active_filters.remove(&payload.0)
    };
    Self::update_sorting(ctx, &(), data);

    false
  }

  #[allow(clippy::trivially_copy_pass_by_ref)]
  fn on_filter_reset(
    _table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    (): &(),
    data: &mut ModList,
  ) -> bool {
    data.filter_state.active_filters.clear();
    Self::update_sorting(ctx, &(), data);
    ctx.request_update();

    true
  }

  pub async fn parse_mod_folder_inner(
    root_dir: &Path,
  ) -> anyhow::Result<(RawModMap, AHashSet<String>)> {
    let client = Arc::new(WebClient::new());

    let mod_dir = root_dir.join("mods");
    let enabled_mods_filename = mod_dir.join("enabled_mods.json");

    let enabled_mods = if enabled_mods_filename.exists()
      && let Ok(enabled_mods_text) = std::fs::read_to_string(enabled_mods_filename)
      && let Ok(EnabledMods { enabled_mods }) =
        serde_json::from_str::<EnabledMods>(&enabled_mods_text)
    {
      Arc::new(enabled_mods)
    } else {
      Default::default()
    };

    tokio_stream::wrappers::ReadDirStream::new(tokio::fs::read_dir(mod_dir).await?)
      .map(|entry| {
        let enabled_mods = enabled_mods.clone();
        let client = client.clone();
        tokio::spawn(async move {
          let Ok(entry) = entry else {
            return None;
          };

          if tokio::fs::metadata(entry.path())
            .await
            .map(|meta| meta.is_dir())
            .unwrap_or_default()
          {
            match RawModEntry::from_file(&entry.path(), ModMetadata::default()) {
              Ok(mut entry) => {
                entry.set_enabled(enabled_mods.contains(&entry.mod_id));

                entry.spawn_version_check(&client);

                if ModMetadata::path(&entry.path).exists() {
                  if let Some(mod_metadata) =
                    ModMetadata::parse_and_send(entry.mod_id.clone(), entry.path.clone(), None)
                      .await
                  {
                    entry.manager_metadata = mod_metadata;
                  }
                }
                Some(entry)
              }
              Err(err) => {
                eprintln!("Failed to get mod info for mod at: {:?}", entry.path());
                eprintln!("With err: {err:?}");
                None
              }
            }
          } else {
            None
          }
        })
      })
      .buffer_unordered(
        std::thread::available_parallelism()
          .map(NonZero::get)
          .unwrap_or(8),
      )
      .try_fold(
        (FastImMap::new().inner(), AHashSet::new()),
        |(mods, mut dupe_ids), entry| async move {
          let Some(entry) = entry else {
            return Ok((mods, dupe_ids));
          };

          Ok((
            mods.update_with(entry.mod_id.clone(), entry, |mut old, new| {
              let dupes = Arc::make_mut(&mut old.duplicates);

              dupe_ids.insert(old.mod_id.clone());

              dupes.push(new);

              old
            }),
            dupe_ids,
          ))
        },
      )
      .await
      .map(|(map, dupes)| (map.into(), dupes))
      .context("Join err")
  }

  pub async fn parse_mod_folder_async(root_dir: PathBuf, ext_ctx: ExtEventSink) {
    let map = Self::parse_mod_folder_inner(&root_dir).await;

    let (mods, duplicates) = match map {
      Ok(res) => res,
      Err(err) => {
        eprintln!("{} | Failed to parse mod folder async: {err}", line!());
        return;
      }
    };
    if let Err(err) = ext_ctx.submit_command_global(super::App::REPLACE_MODS, SingleUse::new(mods))
    {
      eprintln!("{} | {err}", line!());
    }
    for dupe in duplicates {
      let _ =
        ext_ctx.submit_command_global(super::Popup::QUEUE_POPUP, super::Popup::duplicate(dupe));
    }
  }

  pub fn sorted_vals(
    mods: &FastImMap<String, Rc<ModEntry>>,
    header: &Header,
    search_text: &str,
    filters: &[Filters],
  ) -> Vec<String> {
    comemo::evict(20);

    Self::sorted_vals_memo(mods, header, search_text, filters)
  }

  #[memoize]
  fn sorted_vals_memo(
    mods: &FastImMap<String, Rc<ModEntry>>,
    header: &Header,
    search_text: &str,
    filters: &[Filters],
  ) -> Vec<String> {
    Self::sorted_vals_inner(mods, header, search_text, filters)
  }

  pub fn sorted_vals_inner(
    mods: &FastImMap<String, Rc<ModEntry>>,
    header: &Header,
    search_text: &str,
    filters: &[Filters],
  ) -> Vec<String> {
    let mut ids: Vec<_> = mods
      .values()
      .filter_map(|entry| {
        let search = if let Heading::Score = header.sort_by.0 {
          search_text.is_empty() || {
            let id_score = best_match(search_text, &entry.mod_id).map(|m| m.score());
            let name_score = best_match(search_text, &entry.name).map(|m| m.score());
            let author_score = best_match(search_text, entry.author.as_deref().unwrap_or_default())
              .map(|m| m.score());

            id_score.is_some() || name_score.is_some() || author_score.is_some()
          }
        } else {
          true
        };

        let matches_all = filters
          .split(Filters::is_version_checker_filter)
          .flatten()
          .all(|f| f.as_fn()(entry));
        let matches_any = filters
          .split(|filter| !filter.is_version_checker_filter())
          .flatten()
          .any(|f| f.as_fn()(entry));
        let passes_filters = matches_all && (filters.is_empty() || matches_any);

        (search && passes_filters).then(|| entry.mod_id.clone())
      })
      .collect();

    macro_rules! sort {
      ($ids:ident, $field:ident) => {{
        $ids.sort_unstable_by_key(|id| {
          let entry = &mods[id];
          &entry.$field
        });
      }};
      ($ids:ident, $i:ident $t:ty $e:block) => {{
        $ids.sort_unstable_by(|id, other| {
          fn map($i: &ModEntry) -> $t $e

          let entry: &ModEntry = &mods[id];
          let other: &ModEntry = &mods[other];
          map(entry).cmp(&map(other))
        })
      }};
      ($ids:ident, key $e:expr) => {{
        $ids.sort_unstable_by_key(|id| {
          let entry: &ModEntry = &mods[id];
          $e(entry)
        })
      }};
      ($ids:ident, cached $e:expr) => {{
        $ids.sort_by_cached_key(|id| {
          let entry: &ModEntry = &mods[id];
          $e(entry)
        })
      }};
    }

    match header.sort_by.0 {
      Heading::ID => ids.sort_unstable(),
      Heading::Name => sort!(ids, name),
      Heading::Author => sort!(ids, author),
      Heading::GameVersion => sort!(ids, game_version),
      Heading::Enabled => sort!(ids, enabled),
      Heading::Version => sort!(ids, entry Result<&String, &String> {
        entry.version_checker.as_ref().map_or_else(|| Ok(&entry.name), |_| Err(&entry.name))
      }),
      Heading::Score => sort!(ids, cached |entry: &ModEntry| {
        let id_score = best_match(search_text, &entry.mod_id).map(|m| m.score());
        let name_score = best_match(search_text, &entry.name).map(|m| m.score());
        let author_score =
          best_match(search_text, entry.author.as_deref().unwrap_or_default()).map(|m| m.score());

        id_score
          .max(name_score)
          .max(author_score)
          .ok_or_else(|| entry.name.clone())
      }),
      Heading::AutoUpdateSupport => sort!(ids, entry Result<&UpdateStatus, &String> {
        entry.get_direct_download_url().and(entry.version_checker.as_ref().map(|vc| &vc.update_status)).ok_or_else(|| &entry.name)
      }),
      Heading::InstallDate => {
        sort!(ids, key |entry: &ModEntry| entry.manager_metadata.install_date)
      }
      Heading::Type => sort!(ids, key |entry: &ModEntry| {
        if entry.total_conversion {
          3
        } else if entry.utility {
          2
        } else {
          1
        }
      }),
    };

    if header.sort_by.1 {
      ids.reverse();
    }
    ids
  }
}

const BOLD_TEXT_STYLE: (druid::Key<f64>, druid::FontWeight, druid::Key<druid::Color>) = (
  druid::theme::TEXT_SIZE_NORMAL,
  druid::FontWeight::SEMI_BOLD,
  druid::theme::DISABLED_TEXT_COLOR,
);

fn fixed_bold_text<T: Data>(text: &str) -> Label<T> {
  common::labels::bold_text(
    text,
    BOLD_TEXT_STYLE.0,
    BOLD_TEXT_STYLE.1,
    BOLD_TEXT_STYLE.2,
  )
}

fn lensed_bold_text<T: Data + ToString>() -> Label<T> {
  common::labels::lensed_bold(BOLD_TEXT_STYLE.0, BOLD_TEXT_STYLE.1, BOLD_TEXT_STYLE.2)
}

impl<I: AsRef<str>> Index<I> for ModList {
  type Output = ModEntry;

  fn index(&self, index: I) -> &Self::Output {
    &self.mods[index.as_ref()]
  }
}

impl TableData for ModList {
  type Column = Heading;
  type Row = ModEntry;

  fn keys(&self) -> impl Iterator<Item = String> {
    ModList::sorted_vals(
      &self.mods,
      &self.header,
      &self.search_text,
      self.filter_state.active_filters.as_ref(),
    )
    .into_iter()
  }

  fn columns(&self) -> impl Iterator<Item = Self::Column> {
    [Heading::Enabled]
      .iter()
      .chain(self.header.headings.iter())
      .copied()
  }

  fn with_mut(&mut self, idx: <Self::Row as RowData>::Id, mutate: impl FnOnce(&mut Self::Row)) {
    let entry = Rc::make_mut(&mut self.mods[&idx]);
    mutate(entry);
  }

  fn index(&self, idx: <Self::Row as RowData>::Id) -> &Self::Row {
    &self.mods[&idx]
  }
}

#[derive(Serialize, Deserialize)]
pub struct EnabledMods {
  #[serde(rename = "enabledMods")]
  enabled_mods: HashSet<String>,
}

impl EnabledMods {
  pub fn save(self, path: &Path) -> Result<(), SaveError> {
    use std::{fs, io::Write};

    let json = serde_json::to_string_pretty(&self).map_err(|_| SaveError::Format)?;

    let mut file =
      fs::File::create(path.join("mods").join("enabled_mods.json")).map_err(|_| SaveError::File)?;

    file
      .write_all(json.as_bytes())
      .map_err(|_| SaveError::Write)
  }
}

impl<T> From<Vec<RawModEntry<T>>> for EnabledMods {
  fn from(from: Vec<RawModEntry<T>>) -> Self {
    Self {
      enabled_mods: from.iter().map(|v| v.mod_id.clone()).collect(),
    }
  }
}

impl From<Vec<String>> for EnabledMods {
  fn from(enabled_mods: Vec<String>) -> Self {
    Self {
      enabled_mods: enabled_mods.into_iter().collect(),
    }
  }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Data, EnumIter, Display, Debug, Default, EnumCount)]
pub enum Filters {
  #[default]
  Enabled,
  Disabled,
  Unimplemented,
  Error,
  Discrepancy,
  #[strum(to_string = "Up To Date")]
  UpToDate,
  Patch,
  Minor,
  Major,
  #[strum(to_string = "Auto Update Available")]
  AutoUpdateAvailable,
  #[strum(to_string = "Auto Update Unsupported")]
  AutoUpdateUnsupported,
}

impl Filters {
  fn is_version_checker_filter(&self) -> bool {
    matches!(
      self,
      Self::Unimplemented
        | Self::Error
        | Self::Discrepancy
        | Self::UpToDate
        | Self::Patch
        | Self::Minor
        | Self::Major
    )
  }

  fn as_fn(self) -> impl FnMut(&ModEntry) -> bool {
    match self {
      Filters::Enabled => |entry: &ModEntry| !entry.enabled,
      Filters::Disabled => |entry: &ModEntry| entry.enabled,
      Filters::Unimplemented => |entry: &ModEntry| entry.version_checker.is_none(),
      Filters::Error => |entry: &ModEntry| {
        entry.version_checker.as_ref().map(|vc| &vc.update_status) == Some(&UpdateStatus::Error)
      },
      Filters::UpToDate => |entry: &ModEntry| {
        entry.version_checker.as_ref().map(|vc| &vc.update_status) == Some(&UpdateStatus::UpToDate)
      },
      Filters::Discrepancy => |entry: &ModEntry| {
        matches!(
          entry.version_checker.as_ref().map(|vc| &vc.update_status),
          Some(UpdateStatus::Discrepancy(_))
        )
      },
      Filters::Patch => |entry: &ModEntry| {
        matches!(
          entry.version_checker.as_ref().map(|vc| &vc.update_status),
          Some(UpdateStatus::Patch(_))
        )
      },
      Filters::Minor => |entry: &ModEntry| {
        matches!(
          entry.version_checker.as_ref().map(|vc| &vc.update_status),
          Some(UpdateStatus::Minor(_))
        )
      },
      Filters::Major => |entry: &ModEntry| {
        matches!(
          entry.version_checker.as_ref().map(|vc| &vc.update_status),
          Some(UpdateStatus::Major(_))
        )
      },
      Filters::AutoUpdateAvailable => |entry: &ModEntry| entry.get_direct_download_url().is_some(),
      Filters::AutoUpdateUnsupported => {
        |entry: &ModEntry| entry.get_direct_download_url().is_none()
      }
    }
  }
}

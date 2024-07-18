use std::{
  collections::HashMap,
  hash::Hash,
  iter::FromIterator,
  ops::{Deref, Index},
  path::{Path, PathBuf},
  sync::Arc,
};

use comemo::memoize;
use druid::{
  im::Vector,
  theme,
  widget::{Flex, Painter},
  Data, EventCtx, ExtEventSink, Lens, LensExt, Rect, RenderContext, Selector, SingleUse, Widget,
  WidgetExt,
};
use druid_widget_nursery::{
  Stack, StackChildParams, StackChildPosition, WidgetExt as WidgetExtNursery,
};
use rand::Rng;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};
use sublime_fuzzy::best_match;
use webview_shared::ExtEventSinkExt;

use super::{
  controllers::ExtensibleController,
  installer::HybridPath,
  mod_entry::{
    GameVersion, ModEntry as RawModEntry, ModMetadata, UpdateStatus, ViewModEntry as ModEntry,
  },
  util::{self, FastImMap, SaveError, WidgetExtEx},
  App,
};
use crate::{
  patch::table::{ComplexTableColumnWidth, FlexTable, TableColumnWidth, TableData},
  widgets::card::Card,
};

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

const CONTROL_WIDTH: f64 = 175.0;

#[derive(Clone, Data, Lens)]
pub struct ModList {
  pub mods: FastImMap<String, Arc<ModEntry>>,
  pub header: Header,
  pub search_text: String,
  starsector_version: Option<GameVersion>,
  install_state: InstallState,
  pub filter_state: FilterState,
  pub install_dir_available: bool,
  pub refreshing: bool,
}

impl ModList {
  pub const OVERWRITE: Selector<(PathBuf, HybridPath, RawModEntry)> =
    Selector::new("mod_list.install.overwrite");
  pub const AUTO_UPDATE: Selector<ModEntry> = Selector::new("mod_list.install.auto_update");
  pub const SEARCH_UPDATE: Selector<bool> = Selector::new("mod_list.filter.search.update");
  pub const FILTER_UPDATE: Selector<(Filters, bool)> = Selector::new("mod_list.filter.update");
  pub const FILTER_RESET: Selector = Selector::new("mod_list.filter.reset");
  pub const DUPLICATE: Selector<(ModEntry, ModEntry)> =
    Selector::new("mod_list.submit_entry.duplicate");

  pub const REBUILD: Selector = Selector::new("mod_list.table.rebuild");
  pub const REBUILD_NEXT_PASS: Selector = Selector::new("mod_list.table.rebuild_next_pass");
  pub const UPDATE_COLUMN_WIDTH: Selector<(usize, f64)> =
    Selector::new("mod_list.column.update_width");
  const UPDATE_TABLE_SORT: Selector = Selector::new("mod_list.table.update_sorting");
  pub const INSERT_MOD: Selector<RawModEntry> = Selector::new("mod_list.mods.insert");

  pub fn new(headings: Vector<Heading>) -> Self {
    Self {
      mods: FastImMap::new(),
      header: Header::new(headings),
      search_text: String::new(),
      starsector_version: None,
      install_state: InstallState::default(),
      filter_state: Default::default(),
      install_dir_available: false,
      refreshing: false,
    }
  }

  pub fn replace_mods(&mut self, mods: FastImMap<String, RawModEntry>) {
    *self.mods = druid::im::HashMap::from_iter(
      mods
        .inner()
        .into_iter()
        .map(|(id, entry)| (id, Arc::new(ModEntry::from(entry)))),
    );
  }

  pub fn view() -> impl Widget<Self> {
    Stack::new()
      .with_child(
        Flex::column()
          .with_child(
            Flex::row()
              .with_child(
                InstallButton::view()
                  .lens(Self::install_state)
                  .padding((0.0, 5.0))
                  .disabled_if(|data, _| !data.install_dir_available),
              )
              .with_child(Refresh::view().padding((0.0, 5.0)))
              .with_flex_spacer(1.0)
              .with_child(FilterButton::view().lens(Self::filter_state))
              .with_child(
                Search::view()
                  .lens(Self::search_text)
                  .on_change(|ctx, old, data, _| {
                    if !old.same(data) {
                      if data.search_text.is_empty() {
                        data.header.sort_by = (Heading::Name, true)
                      } else {
                        data.header.sort_by = (Heading::Score, true)
                      };
                      ctx.submit_command(Self::UPDATE_TABLE_SORT)
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
                          ctx.fill(rect, &env.get(theme::BACKGROUND_DARK))
                        } else {
                          ctx.fill(rect, &env.get(theme::BACKGROUND_LIGHT))
                        }
                      }))
                      .with_column_width(TableColumnWidth::Fixed(Header::ENABLED_WIDTH))
                      .column_border(theme::BORDER_DARK, 1.0)
                      .clip_aware(true)
                      .controller(
                        ExtensibleController::new()
                          .on_command(Self::UPDATE_COLUMN_WIDTH, Self::column_resized)
                          .on_command(Self::UPDATE_TABLE_SORT, |_, ctx, _payload, _data| {
                            Self::update_sorting(ctx, _payload, _data)
                          })
                          .on_command(ModMetadata::SUBMIT_MOD_METADATA, Self::metadata_submitted)
                          .on_command(Self::FILTER_UPDATE, Self::on_filter_change)
                          .on_command(Self::FILTER_RESET, Self::on_filter_reset)
                          .on_command(Self::REBUILD, |table, ctx, _, _| {
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
                              .insert(entry.id.clone(), Arc::new(entry.clone().into()));
                            ctx.request_update();
                            true
                          }),
                      )
                      .in_layout_repeater()
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
        StackChildPosition::default().top(Some(0.0)).left(Some(0.0)),
      )
      .with_positioned_child(
        FilterOptions::view().lens(Self::filter_state),
        StackChildParams::dynamic(|data: &ModList, _| &data.filter_state.stack_position)
          .duration(0.0),
      )
      .with_positioned_child(
        FilterOptions::wide_view().lens(Self::filter_state),
        StackChildPosition::default()
          .top(Some(54.0))
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
      .on_command(App::REFRESH, |_, _, data| {
        data.refreshing = true;
      })
      .on_command(App::REPLACE_MODS, Self::replace_mods_command_handler)
      .on_added(|_, ctx, _, _| ctx.submit_command(App::REFRESH))
  }

  fn replace_mods_command_handler(
    ctx: &mut EventCtx,
    payload: &SingleUse<FastImMap<String, RawModEntry>>,
    data: &mut ModList,
  ) {
    data.refreshing = false;
    data.replace_mods(payload.take().unwrap());

    Self::update_sorting(ctx, &(), data);
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
      .then(ModEntry::manager_metadata.in_arc())
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
        })
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
          .filter_map(|v| v.enabled.then_some(v.id.clone()))
          .collect();
        if let Err(err) = EnabledMods::from(enabled).save(install_dir) {
          eprintln!("{:?}", err)
        };
      }
    }
  }

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

  fn on_filter_reset(
    _table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    _: &(),
    data: &mut ModList,
  ) -> bool {
    data.filter_state.active_filters.clear();
    Self::update_sorting(ctx, &(), data);
    ctx.request_update();

    true
  }

  pub fn parse_mod_folder(
    root_dir: PathBuf,
  ) -> Result<FastImMap<String, RawModEntry>, (FastImMap<String, RawModEntry>, Vec<Vec<RawModEntry>>)>
  {
    eprintln!("parsing mods");
    let handle = tokio::runtime::Handle::current();

    let mod_dir = root_dir.join("mods");
    let enabled_mods_filename = mod_dir.join("enabled_mods.json");

    let enabled_mods = if enabled_mods_filename.exists()
      && let Ok(enabled_mods_text) = std::fs::read_to_string(enabled_mods_filename)
      && let Ok(EnabledMods { enabled_mods }) =
        serde_json::from_str::<EnabledMods>(&enabled_mods_text)
    {
      enabled_mods
    } else {
      vec![]
    };

    let dir_iter = if let Ok(iter) = std::fs::read_dir(mod_dir) {
      iter
    } else {
      return Ok(Default::default());
    };
    let enabled_mods_iter = enabled_mods.par_iter();

    let client = reqwest::Client::builder()
      .connect_timeout(std::time::Duration::from_millis(500))
      .timeout(std::time::Duration::from_millis(500))
      .build()
      .expect("Build reqwest client");
    let mods = dir_iter
      .par_bridge()
      .filter_map(|entry| entry.ok())
      .filter(|entry| {
        if let Ok(file_type) = entry.file_type() {
          file_type.is_dir()
        } else {
          false
        }
      })
      .filter_map(
        |entry| match RawModEntry::from_file(&entry.path(), ModMetadata::default()) {
          Ok(mut mod_info) => {
            mod_info.set_enabled(
              enabled_mods_iter
                .clone()
                .find_any(|id| mod_info.id.clone().eq(*id))
                .is_some(),
            );
            Some(mod_info)
          }
          Err(err) => {
            eprintln!("Failed to get mod info for mod at: {:?}", entry.path());
            eprintln!("With err: {:?}", err);
            None
          }
        },
      )
      .map(|mut entry| {
        if let Some(version) = entry.version_checker.clone() {
          let master_version = handle.block_on(util::get_master_version(&client, None, &version));
          entry.remote_version = master_version.clone();
          entry.update_status = Some(UpdateStatus::from((&version, &master_version)));
        }
        if ModMetadata::path(&entry.path).exists() {
          if let Some(mod_metadata) = handle.block_on(ModMetadata::parse_and_send(
            entry.id.clone(),
            entry.path.clone(),
            None,
          )) {
            entry.manager_metadata = mod_metadata;
          }
        }

        entry
      })
      .collect::<Vec<_>>();

    let mut bucket_map: HashMap<String, Vec<RawModEntry>> = HashMap::new();

    for entry in mods {
      if let Some(bucket) = bucket_map.get_mut(&entry.id) {
        bucket.push(entry)
      } else {
        bucket_map.insert(entry.id.clone(), vec![entry]);
      }
    }

    let (map, duplicates): (Vec<_>, _) = bucket_map
      .into_iter()
      .partition(|(_, bucket)| bucket.len() == 1);

    let mut out = FastImMap::new();
    *out = map
      .into_iter()
      .map(|(id, mut bucket)| (id, bucket.swap_remove(0)))
      .collect();

    if !duplicates.is_empty() {
      let duplicates = duplicates
        .into_iter()
        .map(|(_, bucket)| bucket)
        .inspect(|bucket| {
          let pick = bucket[rand::thread_rng().gen_range(0..bucket.len())].clone();
          out.insert(pick.id.clone(), pick);
        })
        .collect();

      return Err((out, duplicates));
    }

    Ok(out)
  }

  pub async fn parse_mod_folder_async(root_dir: PathBuf, ext_ctx: ExtEventSink) {
    let map = tokio::task::spawn_blocking(|| Self::parse_mod_folder(root_dir)).await;

    let mods = match map {
      Ok(Ok(mods)) => mods,
      Ok(Err((mods, duplicates))) => {
        let _ = ext_ctx.submit_command_global(
          super::Popup::DELAYED_POPUP,
          duplicates
            .into_iter()
            .map(|dupes| super::Popup::duplicate(dupes.into()))
            .collect::<Vec<_>>(),
        );

        mods
      }
      Err(err) => {
        eprintln!("{} | Failed to parse mod folder async: {err}", line!());
        return;
      }
    };
    if let Err(err) = ext_ctx.submit_command_global(super::App::REPLACE_MODS, SingleUse::new(mods))
    {
      eprintln!("{} | {err}", line!())
    }
  }

  pub fn sorted_vals(
    mods: FastImMap<String, Arc<ModEntry>>,
    header: Header,
    search_text: String,
    filters: Vec<Filters>,
  ) -> Vec<String> {
    comemo::evict(20);

    Self::sorted_vals_memo(mods, header, search_text, filters)
  }

  #[memoize]
  fn sorted_vals_memo(
    mods: FastImMap<String, Arc<ModEntry>>,
    header: Header,
    search_text: String,
    filters: Vec<Filters>,
  ) -> Vec<String> {
    Self::sorted_vals_inner(mods, header, search_text, filters)
  }

  pub fn sorted_vals_inner(
    mods: FastImMap<String, Arc<ModEntry>>,
    header: Header,
    search_text: String,
    filters: Vec<Filters>,
  ) -> Vec<String> {
    let mut ids: Vec<_> = mods
      .values()
      .filter_map(|entry| {
        let search = if let Heading::Score = header.sort_by.0 {
          if !search_text.is_empty() {
            let id_score = best_match(&search_text, &entry.id).map(|m| m.score());
            let name_score = best_match(&search_text, &entry.name).map(|m| m.score());
            let author_score =
              best_match(&search_text, entry.author.as_deref().unwrap_or_default())
                .map(|m| m.score());

            id_score.is_some() || name_score.is_some() || author_score.is_some()
          } else {
            true
          }
        } else {
          true
        };
        let filters = filters.iter().all(|f| f.as_fn()(entry));

        (search && filters).then(|| entry.id.clone())
      })
      .collect();

    macro_rules! sort {
      ($ids:ident, $field:ident) => {{
        $ids.sort_unstable_by_key(|id| {
          let entry = &mods[id];
          &entry.$field
        });
      }};
      ($ids:ident, $e:expr) => {{
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
      Heading::Version => sort!(ids, |entry: &ModEntry| {
        entry
          .update_status
          .clone()
          .ok_or_else(|| entry.name.clone())
      }),
      Heading::Score => sort!(ids, |entry: &ModEntry| {
        let id_score = best_match(&search_text, &entry.id).map(|m| m.score());
        let name_score = best_match(&search_text, &entry.name).map(|m| m.score());
        let author_score =
          best_match(&search_text, entry.author.as_deref().unwrap_or_default()).map(|m| m.score());

        id_score
          .max(name_score)
          .max(author_score)
          .ok_or_else(|| entry.name.clone())
      }),
      Heading::AutoUpdateSupport => sort!(ids, |entry: &ModEntry| {
        entry
          .remote_version
          .clone()
          .and_then(|r| r.direct_download_url.clone())
          .ok_or_else(|| entry.name.clone())
      }),
      Heading::InstallDate => sort!(ids, |entry: &ModEntry| entry.manager_metadata.install_date),
      Heading::Type => sort!(ids, |entry: &ModEntry| {
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
      ids.reverse()
    }
    ids
  }
}

impl<I: AsRef<str>> Index<I> for ModList {
  type Output = ModEntry;

  fn index(&self, index: I) -> &Self::Output {
    &self.mods[index.as_ref()]
  }
}

impl TableData for ModList {
  type Row = ModEntry;
  type Column = Heading;

  fn keys(&self) -> impl Iterator<Item = String> {
    ModList::sorted_vals(
      self.mods.clone(),
      self.header.clone(),
      self.search_text.clone(),
      self.filter_state.active_filters.iter().cloned().collect(),
    )
    .into_iter()
  }

  fn columns(&self) -> impl Iterator<Item = Self::Column> {
    [Heading::Enabled]
      .iter()
      .chain(self.header.headings.iter())
      .cloned()
  }

  fn with_mut(
    &mut self,
    idx: <Self::Row as crate::patch::table::RowData>::Id,
    mutate: impl FnOnce(&mut Self::Row),
  ) {
    let entry = Arc::make_mut(&mut self.mods[&idx]);
    mutate(entry)
  }
}

#[derive(Serialize, Deserialize)]
pub struct EnabledMods {
  #[serde(rename = "enabledMods")]
  enabled_mods: Vec<String>,
}

impl EnabledMods {
  pub fn empty() -> Self {
    Self {
      enabled_mods: Vec::new(),
    }
  }

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
      enabled_mods: from.iter().map(|v| v.id.clone()).collect(),
    }
  }
}

impl From<Vec<String>> for EnabledMods {
  fn from(enabled_mods: Vec<String>) -> Self {
    Self { enabled_mods }
  }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Data, EnumIter, Display, Debug, Default)]
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
  fn as_fn(&self) -> impl FnMut(&ModEntry) -> bool {
    match self {
      Filters::Enabled => |entry: &ModEntry| !entry.enabled,
      Filters::Disabled => |entry: &ModEntry| entry.enabled,
      Filters::Unimplemented => |entry: &ModEntry| entry.version_checker.is_some(),
      Filters::Error => |entry: &ModEntry| entry.update_status != Some(UpdateStatus::Error),
      Filters::UpToDate => |entry: &ModEntry| entry.update_status == Some(UpdateStatus::UpToDate),
      Filters::Discrepancy => {
        |entry: &ModEntry| matches!(entry.update_status, Some(UpdateStatus::Discrepancy(_)))
      }
      Filters::Patch => {
        |entry: &ModEntry| matches!(entry.update_status, Some(UpdateStatus::Patch(_)))
      }
      Filters::Minor => {
        |entry: &ModEntry| matches!(entry.update_status, Some(UpdateStatus::Minor(_)))
      }
      Filters::Major => {
        |entry: &ModEntry| matches!(entry.update_status, Some(UpdateStatus::Major(_)))
      }
      Filters::AutoUpdateAvailable => |entry: &ModEntry| {
        entry
          .remote_version
          .as_ref()
          .and_then(|r| r.direct_download_url.as_ref())
          .is_some()
      },
      Filters::AutoUpdateUnsupported => |entry: &ModEntry| {
        entry
          .remote_version
          .as_ref()
          .and_then(|r| r.direct_download_url.as_ref())
          .is_none()
      },
    }
  }
}

use std::{
  ops::{Deref, Index, IndexMut},
  path::{Path, PathBuf},
};

use druid::{
  im::Vector, theme,
  widget::{Flex, Painter}, Data, EventCtx, ExtEventSink, Lens, LensExt, Rect,
  RenderContext, Selector, SingleUse, Target, Widget, WidgetExt,
};
use druid_widget_nursery::{
  RequestCtx, Stack, StackChildParams, StackChildPosition, WidgetExt as WidgetExtNursery,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};
use sublime_fuzzy::best_match;
use tap::Tap;

use super::{
  controllers::ExtensibleController,
  installer::HybridPath,
  mod_entry::{
    GameVersion, ModEntry as RawModEntry, ModMetadata, UpdateStatus, ViewModEntry as ModEntry,
  },
  util::{self, xxHashMap, SaveError, WidgetExtEx},
  App,
};
use crate::{
  patch::table::{
    ComplexTableColumnWidth, FlexTable, RowData, TableColumnWidth, TableData,
  },
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
  pub mods: xxHashMap<String, ModEntry>,
  pub header: Header,
  search_text: String,
  starsector_version: Option<GameVersion>,
  install_state: InstallState,
  pub filter_state: FilterState,
  pub install_dir_available: bool,
}

impl ModList {
  pub const SUBMIT_ENTRY: Selector<Vec<ModEntry>> = Selector::new("mod_list.submit_entry");
  pub const OVERWRITE: Selector<(PathBuf, HybridPath, RawModEntry)> =
    Selector::new("mod_list.install.overwrite");
  pub const AUTO_UPDATE: Selector<ModEntry> = Selector::new("mod_list.install.auto_update");
  pub const SEARCH_UPDATE: Selector<bool> = Selector::new("mod_list.filter.search.update");
  pub const FILTER_UPDATE: Selector<(Filters, bool)> = Selector::new("mod_list.filter.update");
  pub const FILTER_RESET: Selector = Selector::new("mod_list.filter.reset");
  pub const DUPLICATE: Selector<(ModEntry, ModEntry)> =
    Selector::new("mod_list.submit_entry.duplicate");

  pub const UPDATE_COLUMN_WIDTH: Selector<(usize, f64)> =
    Selector::new("mod_list.column.update_width");
  const UPDATE_TABLE_SORT: Selector = Selector::new("mod_list.table.update_sorting");

  pub fn new(headings: Vector<Heading>) -> Self {
    Self {
      mods: xxHashMap::new(),
      header: Header::new(headings),
      search_text: String::new(),
      starsector_version: None,
      install_state: InstallState::default(),
      filter_state: Default::default(),
      install_dir_available: false,
    }
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
                  .on_change(|ctx, old, data, _| {
                    if !old.same(&data) {
                      ctx.submit_command(Self::SEARCH_UPDATE.with(!data.is_empty()));
                      ctx.submit_command(Self::UPDATE_TABLE_SORT)
                    }
                  })
                  .scope(|curr| Search::new(curr), Search::buffer)
                  .lens(Self::search_text),
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
                      .controller(
                        ExtensibleController::new()
                          .on_command(Self::UPDATE_COLUMN_WIDTH, Self::column_resized)
                          .on_command(Self::UPDATE_TABLE_SORT, |_, ctx, _, data| {
                            Self::update_sorting(ctx, data)
                          })
                          .on_command(Self::SUBMIT_ENTRY, Self::entry_submitted)
                          .on_command(Self::SEARCH_UPDATE, |_, _, searching, data| {
                            if *searching {
                              data.header.sort_by = (Heading::Score, true)
                            } else {
                              data.header.sort_by = (Heading::Name, true)
                            };
                            false
                          })
                          .on_command(ModMetadata::SUBMIT_MOD_METADATA, Self::metadata_submitted)
                          .on_command(Self::FILTER_UPDATE, Self::on_filter_change)
                          .on_command(Self::FILTER_RESET, Self::on_filter_reset)
                          .on_command(App::REPLACE_MODS, Self::replace_mods),
                      )
                      .scroll()
                      .vertical()
                      .expand_width(),
                    1.0,
                  )
                  .on_change(|ctx, old, data, _| {
                    if !old.header.same(&data.header) || !old.mods.same(&data.mods) {
                      Self::update_sorting(ctx, data);
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
  }

  fn entry_submitted(
    _table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    payload: &Vec<ModEntry>,
    data: &mut ModList,
  ) -> bool {
    for entry in payload {
      *data.mods = data.mods.alter(
        |existing| {
          if let Some(inner) = &existing {
            ctx.submit_command(ModList::DUPLICATE.with((inner.clone(), entry.clone())));
            existing
          } else {
            Some(entry.clone())
          }
        },
        entry.id.clone(),
      );
    }
    ctx.children_changed();
    false
  }

  fn replace_mods(
    _table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    payload: &SingleUse<xxHashMap<String, RawModEntry>>,
    data: &mut ModList,
  ) -> bool {
    data.mods = payload
      .take()
      .unwrap()
      .inner()
      .into_iter()
      .map(|(k, v)| (k, ModEntry::from(v)))
      .collect::<druid::im::HashMap<_, _>>()
      .into();

    Self::update_sorting(ctx, data);
    ctx.children_changed();
    dbg!("Finished replacing mods");
    false
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
      .then(ModEntry::manager_metadata)
      .put(data, metadata.clone());

    false
  }

  fn column_resized(
    table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    payload: &(usize, f64),
    _data: &mut ModList,
  ) -> bool {
    if table.rows().count() > 0 {
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

  fn update_sorting(
    ctx: &mut impl RequestCtx,
    data: &mut ModList,
  ) -> bool {
    data.filter_state.sorted_ids = data.sorted_vals().cloned().collect();

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
    Self::update_sorting(ctx, data);

    false
  }

  fn on_filter_reset(
    _table: &mut FlexTable<ModList>,
    ctx: &mut EventCtx,
    _: &(),
    data: &mut ModList,
  ) -> bool {
    data.filter_state.active_filters.clear();
    Self::update_sorting(ctx, data);
    ctx.request_update();

    true
  }

  pub async fn parse_mod_folder(
    event_sink: Option<ExtEventSink>,
    root_dir: Option<PathBuf>,
  ) -> Option<Vec<RawModEntry>> {
    eprintln!("parsing mods");
    let handle = tokio::runtime::Handle::current();

    if let Some(root_dir) = root_dir {
      let mod_dir = root_dir.join("mods");
      let enabled_mods_filename = mod_dir.join("enabled_mods.json");

      let enabled_mods = if !enabled_mods_filename.exists() {
        vec![]
      } else if let Ok(enabled_mods_text) = std::fs::read_to_string(enabled_mods_filename)
        && let Ok(EnabledMods { enabled_mods }) =
          serde_json::from_str::<EnabledMods>(&enabled_mods_text)
      {
        enabled_mods
      } else {
        return None;
      };

      if let Ok(dir_iter) = std::fs::read_dir(mod_dir) {
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
              let master_version =
                handle.block_on(util::get_master_version(&client, None, version.clone()));
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
          });

        if let Some(event_sink) = event_sink.as_ref() {
          let map = xxHashMap::new().tap_mut(|map| {
            map.extend(
              mods
                .map(|entry| (entry.id.clone(), entry))
                .collect::<Vec<_>>()
                .into_iter(),
            )
          });

          if let Err(err) =
            event_sink.submit_command(super::App::REPLACE_MODS, SingleUse::new(map), Target::Auto)
          {
            eprintln!("{:?}", err)
          }
        } else {
          return Some(mods.collect::<Vec<_>>());
        }
      }
    }

    None
  }

  pub fn sorted_vals(&self) -> impl Iterator<Item = &String> {
    let mut values: Vec<&ModEntry> = self
      .mods
      .iter()
      .filter_map(|(_, entry)| {
        let search = if let Heading::Score = self.header.sort_by.0 {
          if !self.search_text.is_empty() {
            let id_score = best_match(&self.search_text, &entry.id).map(|m| m.score());
            let name_score = best_match(&self.search_text, &entry.name).map(|m| m.score());
            let author_score = best_match(&self.search_text, &entry.author).map(|m| m.score());

            id_score.is_some() || name_score.is_some() || author_score.is_some()
          } else {
            true
          }
        } else {
          true
        };
        let filters = self
          .filter_state
          .active_filters
          .iter()
          .all(|f| f.as_fn()(entry));

        (search && filters).then(|| entry)
      })
      .collect();

    values.sort_unstable_by(|a, b| {
      let ord = match self.header.sort_by.0 {
        Heading::ID => a.id.cmp(&b.id),
        Heading::Name => a.name.cmp(&b.name),
        Heading::Author => a.author.cmp(&b.author),
        Heading::GameVersion => a.game_version.cmp(&b.game_version),
        Heading::Enabled => a.enabled.cmp(&b.enabled),
        Heading::Version => match (a.update_status.as_ref(), b.update_status.as_ref()) {
          (None, None) => a.name.cmp(&b.name),
          (_, _) if a.update_status.cmp(&b.update_status) == std::cmp::Ordering::Equal => {
            a.name.cmp(&b.name)
          }
          (_, _) => a.update_status.cmp(&b.update_status),
        },
        Heading::Score => {
          let scoring = |entry: &ModEntry| -> Option<isize> {
            let id_score = best_match(&self.search_text, &entry.id).map(|m| m.score());
            let name_score = best_match(&self.search_text, &entry.name).map(|m| m.score());
            let author_score = best_match(&self.search_text, &entry.author).map(|m| m.score());

            std::cmp::max(std::cmp::max(id_score, name_score), author_score)
          };

          scoring(a).cmp(&scoring(b))
        }
        Heading::AutoUpdateSupport => a
          .remote_version
          .as_ref()
          .and_then(|r| r.direct_download_url.as_ref())
          .is_some()
          .cmp(
            &b.remote_version
              .as_ref()
              .and_then(|r| r.direct_download_url.as_ref())
              .is_some(),
          ),
        Heading::InstallDate => a
          .manager_metadata
          .install_date
          .cmp(&b.manager_metadata.install_date),
      };

      if self.header.sort_by.1 {
        ord.reverse()
      } else {
        ord
      }
    });

    values.into_iter().map(|entry| entry.id())
  }
}

impl<I: AsRef<str>> Index<I> for ModList {
  type Output = ModEntry;

  fn index(&self, index: I) -> &Self::Output {
    &self.mods[index.as_ref()]
  }
}

impl<I: AsRef<str>> IndexMut<I> for ModList {
  fn index_mut(&mut self, index: I) -> &mut Self::Output {
    &mut self.mods[index.as_ref()]
  }
}

impl TableData for ModList {
  type Row = ModEntry;
  type Column = Heading;

  fn keys(&self) -> impl Iterator<Item = &String> {
    self.filter_state.sorted_ids.iter()
  }

  fn columns(&self) -> impl Iterator<Item = &Self::Column> {
    [Heading::Enabled].iter().chain(self.header.headings.iter())
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

#[derive(Clone, Copy, Eq, PartialEq, Hash, Data, EnumIter, Display, Debug)]
pub enum Filters {
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

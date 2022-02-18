use std::{
  collections::{BTreeMap, HashSet},
  path::{Path, PathBuf},
  rc::Rc,
  sync::Arc,
};

use druid::{
  lens, theme,
  widget::{Either, Flex, Label, List, ListIter, Painter, Scroll},
  Color, Data, ExtEventSink, KeyOrValue, Lens, LensExt, Rect, RenderContext, Selector, Target,
  Widget, WidgetExt,
};
use druid_widget_nursery::WidgetExt as WidgetExtNursery;
use if_chain::if_chain;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};
use sublime_fuzzy::best_match;

use crate::app::util::StarsectorVersionDiff;

use super::{
  installer::HybridPath,
  mod_entry::{GameVersion, ModEntry, UpdateStatus},
  util::{self, SaveError},
};

pub mod headings;
use self::headings::Headings;

#[derive(Clone, Data, Lens)]
pub struct ModList {
  #[data(same_fn = "PartialEq::eq")]
  pub mods: BTreeMap<String, Arc<ModEntry>>,
  headings: Headings,
  search_text: String,
  #[data(same_fn = "PartialEq::eq")]
  active_filters: HashSet<Filters>,
  starsector_version: Option<GameVersion>,
}

impl ModList {
  pub const SUBMIT_ENTRY: Selector<Arc<ModEntry>> = Selector::new("mod_list.submit_entry");
  pub const OVERWRITE: Selector<(PathBuf, HybridPath, Arc<ModEntry>)> =
    Selector::new("mod_list.install.overwrite");
  pub const AUTO_UPDATE: Selector<Arc<ModEntry>> = Selector::new("mod_list.install.auto_update");
  pub const SEARCH_UPDATE: Selector<()> = Selector::new("mod_list.filter.search.update");
  pub const FILTER_UPDATE: Selector<(Filters, bool)> = Selector::new("mod_list.filter.update");

  pub fn new() -> Self {
    Self {
      mods: BTreeMap::new(),
      headings: Headings::new(&headings::RATIOS),
      search_text: String::new(),
      active_filters: HashSet::new(),
      starsector_version: None,
    }
  }

  pub fn ui_builder() -> impl Widget<Self> {
    Flex::column()
      .with_child(headings::Headings::ui_builder().lens(ModList::headings))
      .with_flex_child(
        Either::new(
          |data: &ModList, _| !data.mods.is_empty(),
          Scroll::new(
            List::new(|| {
              ModEntry::ui_builder()
                .expand_width()
                .lens(lens!(
                  (Arc<ModEntry>, usize, Rc<[f64; 5]>, Rc<Option<GameVersion>>),
                  0
                ))
                .background(Painter::new(
                  |ctx, (entry, i, ratios, game_version): &EntryAlias, env| {
                    let rect = ctx.size().to_rect();
                    // manually paint cells here to indicate version info
                    // set ratios in ModList through a command listener on this widget
                    // implement update status parser
                    // calculate cell widths using ratios and paint appropriately
                    fn calc_pos(idx: usize, ratios: &Rc<[f64; 5]>, width: f64) -> f64 {
                      if idx == 0 {
                        0.
                      } else if idx == 1 {
                        (ratios[idx - 1] * width) + 3.
                      } else {
                        let prev = calc_pos(idx - 1, ratios, width);
                        prev + ((width - prev) * ratios[idx - 1]) + 3.
                      }
                    }

                    if i % 2 == 0 {
                      ctx.fill(rect, &env.get(theme::BACKGROUND_DARK))
                    } else {
                      ctx.fill(rect, &env.get(theme::BACKGROUND_LIGHT))
                    }
                    if let Some(local) = &entry.version_checker {
                      let update_status = UpdateStatus::from((local, &entry.remote_version));

                      let enabled_shift = (headings::ENABLED_RATIO) * rect.width();
                      let mut row_origin = rect.origin();
                      row_origin.x += enabled_shift + 3.;
                      let row_rect = rect.with_origin(row_origin).intersect(rect);

                      let cell_left = calc_pos(3, ratios, row_rect.width());
                      let cell_right = calc_pos(4, ratios, row_rect.width());
                      let cell_0_rect = Rect::from_points(
                        (row_rect.origin().x + cell_left, row_rect.origin().y),
                        (row_rect.origin().x + cell_right, row_rect.height()),
                      );

                      let color = <KeyOrValue<Color>>::from(update_status).resolve(env);
                      ctx.fill(cell_0_rect, &color)
                    }
                    if let Some(game_version) = game_version.as_ref() {
                      let diff = StarsectorVersionDiff::from((&entry.game_version, game_version));
                      let enabled_shift = (headings::ENABLED_RATIO) * rect.width();
                      let mut row_origin = rect.origin();
                      row_origin.x += enabled_shift + 3.;
                      let row_rect = rect.with_origin(row_origin).intersect(rect);

                      let cell_left = calc_pos(5, ratios, row_rect.width());
                      let cell_0_rect = Rect::from_points(
                        (row_rect.origin().x + cell_left, row_rect.origin().y),
                        (row_rect.max_x(), row_rect.height()),
                      );

                      let color = <KeyOrValue<Color>>::from(diff).resolve(env);
                      ctx.fill(cell_0_rect, &color)
                    }
                  },
                ))
            })
            .lens(lens::Identity)
            .background(theme::BACKGROUND_LIGHT)
            .on_command(ModEntry::REPLACE, |ctx, payload, data: &mut ModList| {
              data.mods.insert(payload.id.clone(), payload.clone());
              ctx.children_changed();
            })
            .on_command(ModList::SEARCH_UPDATE, |ctx, _, data| {
              data.headings.sort_by = (Sorting::Score, true);
              ctx.children_changed()
            })
            .on_command(ModList::FILTER_UPDATE, |ctx, (filter, insert), data| {
              if *insert {
                data.active_filters.insert(*filter)
              } else {
                data.active_filters.remove(filter)
              };
              ctx.children_changed()
            }),
          )
          .vertical(),
          Label::new("No mods")
            .expand()
            .background(theme::BACKGROUND_LIGHT),
        ),
        1.,
      )
      .on_command(ModList::SUBMIT_ENTRY, |_ctx, payload, data| {
        let mut payload = payload.clone();
        if let Some(version_checker) = data.mods.get(&payload.id).and_then(|e| e.version_checker.clone()) {
          (*Arc::make_mut(&mut payload)).version_checker = Some(version_checker);
        }
        data.mods.insert(payload.id.clone(), payload);
      })
      .on_command(Headings::SORT_CHANGED, |ctx, payload, data| {
        if data.headings.sort_by.0 == *payload {
          data.headings.sort_by.1 = !data.headings.sort_by.1;
        } else {
          data.headings.sort_by = (*payload, false)
        }
        ctx.children_changed()
      })
      .on_command(util::MASTER_VERSION_RECEIVED, |_ctx, payload, data| {
        if let Some(mut entry) = data.mods.get(&payload.0).cloned() {
          let remote = payload.1.as_ref().ok().cloned();
          ModEntry::remote_version
            .in_arc()
            .put(&mut entry, remote.clone());
          if let Some(version_checker) = &entry.version_checker {
            let status = UpdateStatus::from((version_checker, &remote));
            ModEntry::update_status
              .in_arc()
              .put(&mut entry, Some(status));
          }
          data.mods.insert(entry.id.clone(), entry);
        };
      })
  }

  pub async fn parse_mod_folder(event_sink: ExtEventSink, root_dir: Option<PathBuf>) {
    let handle = tokio::runtime::Handle::current();

    if let Some(root_dir) = root_dir {
      let mod_dir = root_dir.join("mods");
      let enabled_mods_filename = mod_dir.join("enabled_mods.json");

      let enabled_mods = if !enabled_mods_filename.exists() {
        vec![]
      } else {
        if_chain! {
          if let Ok(enabled_mods_text) = std::fs::read_to_string(enabled_mods_filename);
          if let Ok(EnabledMods { enabled_mods }) = serde_json::from_str::<EnabledMods>(&enabled_mods_text);
          then {
            enabled_mods
          } else {
            return
          }
        }
      };

      if let Ok(dir_iter) = std::fs::read_dir(mod_dir) {
        let enabled_mods_iter = enabled_mods.par_iter();

        dir_iter
          .par_bridge()
          .filter_map(|entry| entry.ok())
          .filter(|entry| {
            if let Ok(file_type) = entry.file_type() {
              file_type.is_dir()
            } else {
              false
            }
          })
          .filter_map(|entry| {
            if let Ok(mut mod_info) = ModEntry::from_file(&entry.path()) {
              mod_info.set_enabled(
                enabled_mods_iter
                  .clone()
                  .find_any(|id| mod_info.id.clone().eq(*id))
                  .is_some(),
              );
              Some(Arc::new(mod_info))
            } else {
              dbg!(entry.path());
              None
            }
          })
          .for_each(|entry| {
            if let Err(err) = event_sink.submit_command(ModList::SUBMIT_ENTRY, entry.clone(), Target::Auto)
            {
              eprintln!("Failed to submit found mod {}", err);
            };
            if entry.version_checker.is_some() {
              let event_sink = event_sink.clone();
              handle.spawn(async move {
                util::get_master_version(event_sink, entry.version_checker.as_ref().unwrap()).await;
              });
            }
          });
      }
    }
  }

  fn sorted_vals(&self) -> Vec<Arc<ModEntry>> {
    let values_iter = self.mods.par_iter().filter_map(|(_, entry)| {
      let search = if let Sorting::Score = self.headings.sort_by.0 {
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
      let filters = self.active_filters.par_iter().all(|f| f.as_fn()(entry));

      (search && filters).then(|| entry.clone())
    });

    let mut values: Vec<Arc<ModEntry>> = values_iter.collect();
    values.par_sort_unstable_by(|a, b| {
      let ord = match self.headings.sort_by.0 {
        Sorting::ID => a.id.cmp(&b.id),
        Sorting::Name => a.name.cmp(&b.name),
        Sorting::Author => a.author.cmp(&b.author),
        Sorting::GameVersion => a.game_version.cmp(&b.game_version),
        Sorting::Enabled => a.enabled.cmp(&b.enabled),
        Sorting::Version => match (a.update_status.as_ref(), b.update_status.as_ref()) {
          (None, None) => a.name.cmp(&b.name),
          (_, _) if a.update_status.cmp(&b.update_status) == std::cmp::Ordering::Equal => {
            a.name.cmp(&b.name)
          }
          (_, _) => a.update_status.cmp(&b.update_status),
        },
        Sorting::Score => {
          let scoring = |entry: &Arc<ModEntry>| -> Option<isize> {
            let id_score = best_match(&self.search_text, &entry.id).map(|m| m.score());
            let name_score = best_match(&self.search_text, &entry.name).map(|m| m.score());
            let author_score = best_match(&self.search_text, &entry.author).map(|m| m.score());

            std::cmp::max(std::cmp::max(id_score, name_score), author_score)
          };

          scoring(a).cmp(&scoring(b))
        }
        Sorting::AutoUpdateSupport => a
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
      };

      if self.headings.sort_by.1 {
        ord.reverse()
      } else {
        ord
      }
    });
    values
  }
}

type EntryAlias = (Arc<ModEntry>, usize, Rc<[f64; 5]>, Rc<Option<GameVersion>>);

impl ListIter<EntryAlias> for ModList {
  fn for_each(&self, mut cb: impl FnMut(&EntryAlias, usize)) {
    let ratios = Rc::new(self.headings.ratios);
    let game_version = Rc::new(self.starsector_version.clone());

    for (i, item) in self.sorted_vals().into_iter().enumerate() {
      cb(&(item, i, ratios.clone(), game_version.clone()), i);
    }
  }

  fn for_each_mut(&mut self, mut cb: impl FnMut(&mut EntryAlias, usize)) {
    let ratios = Rc::new(self.headings.ratios);
    let game_version = Rc::new(self.starsector_version.clone());

    for (i, item) in self.sorted_vals().iter_mut().enumerate() {
      cb(
        &mut (item.clone(), i, ratios.clone(), game_version.clone()),
        i,
      );
    }
  }

  fn data_len(&self) -> usize {
    self.mods.len()
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
    use std::fs;
    use std::io::Write;

    let json = serde_json::to_string_pretty(&self).map_err(|_| SaveError::Format)?;

    let mut file =
      fs::File::create(path.join("mods").join("enabled_mods.json")).map_err(|_| SaveError::File)?;

    file
      .write_all(json.as_bytes())
      .map_err(|_| SaveError::Write)
  }
}

impl From<Vec<Arc<ModEntry>>> for EnabledMods {
  fn from(from: Vec<Arc<ModEntry>>) -> Self {
    Self {
      enabled_mods: from.iter().into_iter().map(|v| v.id.clone()).collect(),
    }
  }
}

impl From<Vec<String>> for EnabledMods {
  fn from(enabled_mods: Vec<String>) -> Self {
    Self { enabled_mods }
  }
}

#[derive(Debug, Clone, Copy, Data, PartialEq, Eq)]
pub enum Sorting {
  ID,
  Name,
  Author,
  GameVersion,
  Enabled,
  Version,
  Score,
  AutoUpdateSupport,
}

impl From<Sorting> for &str {
  fn from(sorting: Sorting) -> Self {
    match sorting {
      Sorting::ID => "ID",
      Sorting::Name => "Name",
      Sorting::Author => "Author(s)",
      Sorting::GameVersion => "Game Version",
      Sorting::Enabled => "Enabled",
      Sorting::Version => "Version",
      Sorting::Score => "score",
      Sorting::AutoUpdateSupport => "Auto-Update Supported",
    }
  }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Data, EnumIter, Display)]
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
  fn as_fn(&self) -> impl FnMut(&Arc<ModEntry>) -> bool {
    match self {
      Filters::Enabled => |entry: &Arc<ModEntry>| !entry.enabled,
      Filters::Disabled => |entry: &Arc<ModEntry>| entry.enabled,
      Filters::Unimplemented => |entry: &Arc<ModEntry>| entry.version_checker.is_some(),
      Filters::Error => |entry: &Arc<ModEntry>| {
        !entry
          .update_status
          .is_some_with(|s| s == &UpdateStatus::Error)
      },
      Filters::UpToDate => |entry: &Arc<ModEntry>| {
        !entry
          .update_status
          .is_some_with(|s| s == &UpdateStatus::UpToDate)
      },
      Filters::Discrepancy => |entry: &Arc<ModEntry>| {
        !entry
          .update_status
          .is_some_with(|s| matches!(s, &UpdateStatus::Discrepancy(_)))
      },
      Filters::Patch => |entry: &Arc<ModEntry>| {
        !entry
          .update_status
          .is_some_with(|s| matches!(s, &UpdateStatus::Patch(_)))
      },
      Filters::Minor => |entry: &Arc<ModEntry>| {
        !entry
          .update_status
          .is_some_with(|s| matches!(s, &UpdateStatus::Minor(_)))
      },
      Filters::Major => |entry: &Arc<ModEntry>| {
        !entry
          .update_status
          .is_some_with(|s| matches!(s, &UpdateStatus::Major(_)))
      },
      Filters::AutoUpdateAvailable => |entry: &Arc<ModEntry>| {
        !(entry.update_status.is_some_with(|s| {
          matches!(
            s,
            UpdateStatus::Patch(_) | UpdateStatus::Minor(_) | UpdateStatus::Major(_)
          )
        }) && entry
          .remote_version
          .as_ref()
          .and_then(|r| r.direct_download_url.as_ref())
          .is_some())
      },
      Filters::AutoUpdateUnsupported => |entry: &Arc<ModEntry>| {
        entry
          .remote_version
          .as_ref()
          .and_then(|r| r.direct_download_url.as_ref())
          .is_some()
      },
    }
  }
}

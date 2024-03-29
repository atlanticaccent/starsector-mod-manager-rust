use std::{
  collections::HashSet,
  path::{Path, PathBuf},
  rc::Rc,
  sync::Arc,
};

use druid::{
  im::Vector,
  lens, theme,
  widget::{Either, Flex, Label, List, ListIter, Painter, Scroll},
  Color, Data, ExtEventSink, KeyOrValue, Lens, LensExt, Rect, RenderContext, Selector, Target,
  Widget, WidgetExt,
};
use druid_widget_nursery::WidgetExt as WidgetExtNursery;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};
use sublime_fuzzy::best_match;

use crate::app::util::StarsectorVersionDiff;

use super::{
  installer::HybridPath,
  mod_entry::{GameVersion, ModEntry, ModMetadata, UpdateStatus},
  util::{self, xxHashMap, LoadBalancer, SaveError},
};

pub mod headings;
use self::headings::{Header, Heading};

static UPDATE_BALANCER: LoadBalancer<Arc<ModEntry>, Vec<Arc<ModEntry>>, Vec<Arc<ModEntry>>> =
  LoadBalancer::new(ModList::SUBMIT_ENTRY);

#[derive(Clone, Data, Lens)]
pub struct ModList {
  pub mods: xxHashMap<String, Arc<ModEntry>>,
  pub header: Header,
  search_text: String,
  #[data(same_fn = "PartialEq::eq")]
  active_filters: HashSet<Filters>,
  starsector_version: Option<GameVersion>,
}

impl ModList {
  pub const SUBMIT_ENTRY: Selector<Vec<Arc<ModEntry>>> = Selector::new("mod_list.submit_entry");
  pub const OVERWRITE: Selector<(PathBuf, HybridPath, Arc<ModEntry>)> =
    Selector::new("mod_list.install.overwrite");
  pub const AUTO_UPDATE: Selector<Arc<ModEntry>> = Selector::new("mod_list.install.auto_update");
  pub const SEARCH_UPDATE: Selector<()> = Selector::new("mod_list.filter.search.update");
  pub const FILTER_UPDATE: Selector<(Filters, bool)> = Selector::new("mod_list.filter.update");
  pub const DUPLICATE: Selector<(Arc<ModEntry>, Arc<ModEntry>)> =
    Selector::new("mod_list.submit_entry.duplicate");

  pub fn new(headings: Vector<Heading>) -> Self {
    Self {
      mods: xxHashMap::new(),
      header: Header::new(headings),
      search_text: String::new(),
      active_filters: HashSet::new(),
      starsector_version: None,
    }
  }

  pub fn ui_builder() -> impl Widget<Self> {
    Flex::column()
      .with_child(headings::Header::ui_builder().lens(ModList::header))
      .with_flex_child(
        Either::new(
          |data: &ModList, _| !data.mods.is_empty(),
          Scroll::new(
            List::new(|| {
              ModEntry::ui_builder()
                .expand_width()
                .lens(lens::Map::new(
                  |val: &EntryAlias| (val.0.clone(), val.2.clone(), val.3.clone()),
                  |_, _| {},
                ))
                .background(Painter::new(
                  |ctx, (entry, i, ratios, headings, game_version): &EntryAlias, env| {
                    let rect = ctx.size().to_rect();
                    // manually paint cells here to indicate version info
                    // set ratios in ModList through a command listener on this widget
                    // implement update status parser
                    // calculate cell widths using ratios and paint appropriately
                    fn calc_pos(idx: usize, ratios: &Vector<f64>, width: f64) -> f64 {
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
                    if let Some(idx) = headings.index_of(&Heading::Version) {
                      if let Some(local) = &entry.version_checker {
                        let update_status = UpdateStatus::from((local, &entry.remote_version));

                        let enabled_shift = (headings::ENABLED_RATIO) * rect.width();
                        let mut row_origin = rect.origin();
                        row_origin.x += enabled_shift + 3.;
                        let row_rect = rect.with_origin(row_origin).intersect(rect);

                        let cell_left =
                          row_rect.origin().x + calc_pos(idx, ratios, row_rect.width());
                        let cell_right = if idx < ratios.len() {
                          row_rect.origin().x + calc_pos(idx + 1, ratios, row_rect.width())
                        } else {
                          row_rect.max_x()
                        };
                        let cell_0_rect = Rect::from_points(
                          (cell_left, row_rect.origin().y),
                          (cell_right, row_rect.height()),
                        );

                        let color = <KeyOrValue<Color>>::from(&update_status).resolve(env);
                        ctx.fill(cell_0_rect, &color)
                      }
                    }
                    if let Some(idx) = headings.index_of(&Heading::GameVersion) {
                      if let Some(game_version) = game_version.as_ref() {
                        let diff = StarsectorVersionDiff::from((&entry.game_version, game_version));
                        let enabled_shift = (headings::ENABLED_RATIO) * rect.width();
                        let mut row_origin = rect.origin();
                        row_origin.x += enabled_shift + 3.;
                        let row_rect = rect.with_origin(row_origin).intersect(rect);

                        let cell_left =
                          row_rect.origin().x + calc_pos(idx, ratios, row_rect.width());
                        let cell_right = if idx < ratios.len() {
                          row_rect.origin().x + calc_pos(idx + 1, ratios, row_rect.width())
                        } else {
                          row_rect.max_x()
                        };
                        let cell_0_rect = Rect::from_points(
                          (cell_left, row_rect.origin().y),
                          (cell_right, row_rect.height()),
                        );

                        let color = <KeyOrValue<Color>>::from(diff).resolve(env);
                        ctx.fill(cell_0_rect, &color)
                      }
                    }
                  },
                ))
            })
            .background(theme::BACKGROUND_LIGHT)
            .on_command(ModEntry::REPLACE, |ctx, payload, data: &mut ModList| {
              data.mods.insert(payload.id.clone(), payload.clone());
              ctx.children_changed();
            })
            .on_command(ModList::SEARCH_UPDATE, |ctx, _, data| {
              data.header.sort_by = (Heading::Score, true);
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
      .on_command(ModList::SUBMIT_ENTRY, |ctx, payload, data| {
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
      })
      .on_command(Header::SORT_CHANGED, |ctx, payload, data| {
        if data.header.sort_by.0 == *payload {
          data.header.sort_by.1 = !data.header.sort_by.1;
        } else {
          data.header.sort_by = (*payload, false)
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
      .on_command(
        ModMetadata::SUBMIT_MOD_METADATA,
        |_ctx, (id, metadata), data| {
          if let Some(mut entry) = data.mods.remove(id) {
            ModEntry::manager_metadata
              .in_arc()
              .put(&mut entry, metadata.clone());

            data.mods.insert(id.clone(), entry);
          }
        },
      )
  }

  pub async fn parse_mod_folder(event_sink: ExtEventSink, root_dir: Option<PathBuf>) {
    let handle = tokio::runtime::Handle::current();

    if let Some(root_dir) = root_dir {
      let mod_dir = root_dir.join("mods");
      let enabled_mods_filename = mod_dir.join("enabled_mods.json");

      let enabled_mods = if !enabled_mods_filename.exists() {
        vec![]
      } else if let Ok(enabled_mods_text) = std::fs::read_to_string(enabled_mods_filename)
        && let Ok(EnabledMods { enabled_mods }) = serde_json::from_str::<EnabledMods>(&enabled_mods_text)
      {
        enabled_mods
      } else {
        return
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
            if let Ok(mut mod_info) = ModEntry::from_file(&entry.path(), ModMetadata::default()) {
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
            let tx = {
              let _guard = handle.enter();

              UPDATE_BALANCER.sender(event_sink.clone())
            };

            if let Err(err) = tx.send(entry.clone()) {
              eprintln!("Failed to submit found mod {}", err);
            };
            if let Some(version) = entry.version_checker.clone() {
              handle.spawn(util::get_master_version(event_sink.clone(), version));
            }
            if ModMetadata::path(&entry.path).exists() {
              handle.spawn(ModMetadata::parse_and_send(
                entry.id.clone(),
                entry.path.clone(),
                event_sink.clone(),
              ));
            }
          });
      }
    }

    if event_sink
      .submit_command(super::App::ENABLE, (), Target::Auto)
      .is_err()
    {
      event_sink
        .submit_command(super::App::ENABLE, (), Target::Auto)
        .unwrap();
    };
  }

  fn sorted_vals(&self) -> Vec<Arc<ModEntry>> {
    let mut values: Vec<Arc<ModEntry>> = self
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
        let filters = self.active_filters.par_iter().all(|f| f.as_fn()(entry));

        (search && filters).then(|| entry.clone())
      })
      .collect();

    values.par_sort_unstable_by(|a, b| {
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
          let scoring = |entry: &Arc<ModEntry>| -> Option<isize> {
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
    values
  }
}

type EntryAlias = (
  Arc<ModEntry>,
  usize,
  Vector<f64>,
  Vector<Heading>,
  Rc<Option<GameVersion>>,
);

impl ListIter<EntryAlias> for ModList {
  fn for_each(&self, mut cb: impl FnMut(&EntryAlias, usize)) {
    let ratios = self.header.ratios.clone();
    let headers = self.header.headings.clone();
    let game_version = Rc::new(self.starsector_version.clone());

    for (i, item) in self.sorted_vals().into_iter().enumerate() {
      cb(
        &(
          item,
          i,
          ratios.clone(),
          headers.clone(),
          game_version.clone(),
        ),
        i,
      );
    }
  }

  fn for_each_mut(&mut self, mut cb: impl FnMut(&mut EntryAlias, usize)) {
    let ratios = self.header.ratios.clone();
    let headers = self.header.headings.clone();
    let game_version = Rc::new(self.starsector_version.clone());

    for (i, item) in self.sorted_vals().iter_mut().enumerate() {
      cb(
        &mut (
          item.clone(),
          i,
          ratios.clone(),
          headers.clone(),
          game_version.clone(),
        ),
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
      enabled_mods: from.iter().map(|v| v.id.clone()).collect(),
    }
  }
}

impl From<Vec<String>> for EnabledMods {
  fn from(enabled_mods: Vec<String>) -> Self {
    Self { enabled_mods }
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
      Filters::Error => |entry: &Arc<ModEntry>| entry.update_status != Some(UpdateStatus::Error),
      Filters::UpToDate => {
        |entry: &Arc<ModEntry>| entry.update_status != Some(UpdateStatus::UpToDate)
      }
      Filters::Discrepancy => {
        |entry: &Arc<ModEntry>| !matches!(entry.update_status, Some(UpdateStatus::Discrepancy(_)))
      }
      Filters::Patch => {
        |entry: &Arc<ModEntry>| !matches!(entry.update_status, Some(UpdateStatus::Patch(_)))
      }
      Filters::Minor => {
        |entry: &Arc<ModEntry>| !matches!(entry.update_status, Some(UpdateStatus::Minor(_)))
      }
      Filters::Major => {
        |entry: &Arc<ModEntry>| !matches!(entry.update_status, Some(UpdateStatus::Major(_)))
      }
      Filters::AutoUpdateAvailable => |entry: &Arc<ModEntry>| {
        matches!(
          entry.update_status,
          None | Some(UpdateStatus::Error) | Some(UpdateStatus::UpToDate)
        ) || (matches!(
          entry.update_status,
          Some(UpdateStatus::Patch(_))
            | Some(UpdateStatus::Minor(_))
            | Some(UpdateStatus::Major(_))
        ) ^ entry
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

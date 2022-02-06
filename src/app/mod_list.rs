use std::{collections::BTreeMap, path::PathBuf, sync::Arc, rc::Rc};

use druid::{Widget, widget::{Scroll, List, ListIter, Painter, Flex, Either, Label, Button, Controller}, lens, WidgetExt, Data, Lens, RenderContext, theme, Selector, ExtEventSink, Target, LensExt, WindowConfig, Env, commands, Color, Rect};
use druid_widget_nursery::WidgetExt as WidgetExtNursery;
use if_chain::if_chain;
use serde::{Serialize, Deserialize};
use sublime_fuzzy::best_match;

use super::{mod_entry::{ModEntry, UpdateStatus}, util::{SaveError, self}, installer::{self, ChannelMessage, StringOrPath, HybridPath}};

pub mod headings;
use self::headings::Headings;

#[derive(Clone, Data, Lens)]
pub struct ModList {
  #[data(same_fn="PartialEq::eq")]
  pub mods: BTreeMap<String, Arc<ModEntry>>,
  headings: Headings,
  search_text: String,
  sort_by: (ModEntryComp, bool),
}

impl ModList {
  pub const SUBMIT_ENTRY: Selector<Arc<ModEntry>> = Selector::new("mod_list.submit_entry");
  pub const OVERWRITE: Selector<(PathBuf, HybridPath, Arc<ModEntry>)> = Selector::new("mod_list.install.overwrite");
  pub const AUTO_UPDATE: Selector<Arc<ModEntry>> = Selector::new("mod_list.install.auto_update");
  pub const SEARCH_UPDATE: Selector<()> = Selector::new("mod_list.filter.search.update");

  pub fn new() -> Self {
    Self {
      mods: BTreeMap::new(),
      headings: Headings::new(&headings::RATIOS),
      search_text: String::new(),
      sort_by: (ModEntryComp::Name, false)
    }
  }

  pub fn ui_builder() -> impl Widget<Self> {
    Flex::column()
      .with_child(headings::Headings::ui_builder().lens(ModList::headings))
      .with_flex_child(
        Either::new(
          |data: &ModList, _| data.mods.len() > 0,
          Scroll::new(
            List::new(|| {
              ModEntry::ui_builder().expand_width().lens(lens!((Arc<ModEntry>, usize, Rc<[f64; 5]>), 0)).background(Painter::new(|ctx, (entry, i, ratios): &(Arc<ModEntry>, usize, Rc<[f64; 5]>), env| {
                let rect = ctx.size().to_rect();
                // manually paint cells here to indicate version info
                // set ratios in ModList through a command listener on this widget
                // implement update status parser
                // calculate cell widths using ratios and paint appropriately
                fn calc_pos(idx: usize, ratios: &Rc<[f64; 5]>, width: f64) -> f64 {
                  return if idx == 0 {
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

                  let cell_left = calc_pos(3, &ratios, row_rect.width());
                  let cell_right = calc_pos(4, &ratios, row_rect.width());
                  let cell_0_rect = Rect::from_points((row_rect.origin().x + cell_left, row_rect.origin().y), (row_rect.origin().x + cell_right, row_rect.height()));

                  ctx.fill(cell_0_rect, &update_status.into() as &Color)
                }
              }))
            }).lens(lens::Identity)
            .background(theme::BACKGROUND_LIGHT)
            .on_command(ModEntry::REPLACE, |ctx, payload, data: &mut ModList| {
              data.mods.insert(payload.id.clone(), payload.clone());
              ctx.children_changed();
            })
            .on_command(ModList::SEARCH_UPDATE, |ctx, _, data| {
              data.sort_by = (ModEntryComp::Score, true);
              ctx.children_changed()
            })
            .controller(InstallController)
          ).vertical(),
          Label::new("No mods").expand().background(theme::BACKGROUND_LIGHT)
        ),
        1.
      )
      .on_command(ModList::SUBMIT_ENTRY, |_ctx, payload, data| {
        data.mods.insert(payload.id.clone(), payload.clone());
      })
      .on_command(util::MASTER_VERSION_RECEIVED, |_ctx, payload, data| {
        if let Ok(meta) = payload.1.clone() {
          if let Some(mut entry) = data.mods.get(&payload.0).cloned() {
            ModEntry::remote_version.in_arc().put(&mut entry, Some(meta.clone()));
            if let Some(version_checker) = &entry.version_checker {
              let status = UpdateStatus::from((version_checker, &Some(meta)));
              ModEntry::update_status.in_arc().put(&mut entry, Some(status));
            }
            data.mods.insert(entry.id.clone(), entry);
          };
        }
      })
  }

  pub async fn parse_mod_folder(event_sink: ExtEventSink, root_dir: Option<PathBuf>) {
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
        let enabled_mods_iter = enabled_mods.iter();

        dir_iter
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
              mod_info.set_enabled(enabled_mods_iter.clone().find(|id| mod_info.id.clone().eq(*id)).is_some());
              Some((
                Arc::new(mod_info.clone()),
                mod_info.version_checker.clone()
              ))
            } else {
              dbg!(entry.path());
              None
            }
          })
          .for_each(|(entry, version)| {
            if let Err(err) = event_sink.submit_command(ModList::SUBMIT_ENTRY, entry, Target::Auto) {
              eprintln!("Failed to submit found mod {}", err);
            };
            if let Some(version) = version {
              tokio::spawn(util::get_master_version(event_sink.clone(), version));
            }
          });

        // self.mods.extend(mods);

        // versions.iter()
        //   .filter_map(|v| v.as_ref())
        //   .map(|v| Command::perform(util::get_master_version(v.clone()), ModListMessage::MasterVersionReceived))
        //   .collect()
      } else {
        // debug_println!("Fatal. Could not parse mods folder. Alert developer");
        
      }
    } else {
      
    }
  }
}

impl ListIter<(Arc<ModEntry>, usize, Rc<[f64; 5]>)> for ModList {
  fn for_each(&self, mut cb: impl FnMut(&(Arc<ModEntry>, usize, Rc<[f64; 5]>), usize)) {
    let rc = Rc::new(self.headings.ratios.clone());

    let mut values: Vec<_> = self.mods.values().cloned().filter(|entry| {
      if let ModEntryComp::Score = self.sort_by.0 {
        if self.search_text.len() > 0 {
          let id_score = best_match(&self.search_text, &entry.id).map(|m| m.score());
          let name_score = best_match(&self.search_text, &entry.name).map(|m| m.score());
          let author_score = best_match(&self.search_text, &entry.author).map(|m| m.score());
  
          return id_score.is_some() || name_score.is_some() || author_score.is_some();
        }
      }

      true
    })
    .collect();
    values.sort_unstable_by(|a, b| {
      let ord = match self.sort_by.0 {
        ModEntryComp::ID => a.author.cmp(&b.author),
        ModEntryComp::Name => a.name.cmp(&b.name),
        ModEntryComp::Author => a.author.cmp(&b.author),
        ModEntryComp::GameVersion => a.game_version.cmp(&b.game_version),
        ModEntryComp::Enabled => a.enabled.cmp(&b.enabled),
        ModEntryComp::Version => a.version.cmp(&b.version),
        ModEntryComp::Score => {
          let scoring = |entry: &Arc<ModEntry>| -> Option<isize> {
            let id_score = best_match(&self.search_text, &entry.id).map(|m| m.score());
            let name_score = best_match(&self.search_text, &entry.name).map(|m| m.score());
            let author_score = best_match(&self.search_text, &entry.author).map(|m| m.score());

            std::cmp::max(std::cmp::max(id_score, name_score), author_score)
          };

          scoring(a).cmp(&scoring(b))
        },
        ModEntryComp::AutoUpdateSupport => {
          a.remote_version.as_ref().and_then(|r| r.direct_download_url.as_ref()).is_some()
            .cmp(&b.remote_version.as_ref().and_then(|r| r.direct_download_url.as_ref()).is_some())
        },
      };

      if self.sort_by.1 {
        ord.reverse()
      } else {
        ord
      }
    });

    for (i, item) in values.into_iter().enumerate() {
      cb(&(item, i, rc.clone()), i);
    }
  }
  
  fn for_each_mut(&mut self, mut cb: impl FnMut(&mut (Arc<ModEntry>, usize, Rc<[f64; 5]>), usize)) {
    let rc = Rc::new(self.headings.ratios.clone());

    let mut values: Vec<_> = self.mods.values().cloned().filter(|entry| {
      if let ModEntryComp::Score = self.sort_by.0 {
        if self.search_text.len() > 0 {
          return entry.search_score.is_some()
        }
      }

      true
    })
    .collect();
    values.sort_unstable_by(|a, b| {
      let ord = match self.sort_by.0 {
        ModEntryComp::ID => a.author.cmp(&b.author),
        ModEntryComp::Name => a.name.cmp(&b.name),
        ModEntryComp::Author => a.author.cmp(&b.author),
        ModEntryComp::GameVersion => a.game_version.cmp(&b.game_version),
        ModEntryComp::Enabled => a.enabled.cmp(&b.enabled),
        ModEntryComp::Version => a.version.cmp(&b.version),
        ModEntryComp::Score => a.search_score.cmp(&b.search_score),
        ModEntryComp::AutoUpdateSupport => {
          a.remote_version.as_ref().and_then(|r| r.direct_download_url.as_ref()).is_some()
            .cmp(&b.remote_version.as_ref().and_then(|r| r.direct_download_url.as_ref()).is_some())
        },
      };

      if self.sort_by.1 {
        ord.reverse()
      } else {
        ord
      }
    });

    for (i, item) in values.iter_mut().enumerate() {
      cb(&mut (item.clone(), i, rc.clone()), i);
    }
  }
  
  fn data_len(&self) -> usize {
    self.mods.len()
  }
}

struct InstallController;

impl<W: Widget<ModList>> Controller<ModList, W> for InstallController {
  fn event(&mut self, child: &mut W, ctx: &mut druid::EventCtx, event: &druid::Event, mod_list: &mut ModList, env: &Env) {
    if let druid::Event::Command(cmd) = event {
      if let Some(payload) = cmd.get(installer::INSTALL) {
        match payload {
          ChannelMessage::Success(entry) => {
            mod_list.mods.insert(entry.id.clone(), entry.clone());
            ctx.children_changed();
            println!("Successfully installed {}", entry.id.clone())
          },
          ChannelMessage::Duplicate(conflict, to_install, entry) => {
            let widget = Flex::column()
              .with_child(Label::new(format!("Encountered conflict when trying to install {}", entry.id)))
              .with_child(Label::new(match conflict {
                StringOrPath::String(id) => format!("A mod with ID {} alread exists.", id),
                StringOrPath::Path(path) => format!("A folder already exists at the path {}.", path.to_string_lossy()),
              }))
              .with_child(Label::new(format!("Would you like to replace the existing {}?", if let StringOrPath::String(_) = conflict { "mod" } else { "folder" })))
              .with_default_spacer()
              .with_child(
                Flex::row()
                  .with_child(Button::new("Overwrite").on_click({
                    let conflict = match conflict {
                      StringOrPath::String(id) => mod_list.mods.get(id).unwrap().path.clone(),
                      StringOrPath::Path(path) => path.clone(),
                    };
                    let to_install = to_install.clone();
                    let entry = entry.clone();
                    move |ctx, _, _| {
                      ctx.submit_command(commands::CLOSE_WINDOW);
                      ctx.submit_command(ModList::OVERWRITE.with((conflict.clone(), to_install.clone(), entry.clone())).to(Target::Global))
                    }
                  }))
                  .with_child(Button::new("Cancel").on_click(|ctx, _, _| {
                    ctx.submit_command(commands::CLOSE_WINDOW)
                  }))
              ).cross_axis_alignment(druid::widget::CrossAxisAlignment::Start);

            ctx.new_sub_window(
              WindowConfig::default().resizable(true).window_size((500.0, 200.0)),
              widget,
              mod_list.clone(),
              env.clone()
            );
          },
          ChannelMessage::Error(err) => {
            eprintln!("Failed to install {}", err);
          }
        }
      }
    } else if let druid::Event::Notification(notif) = event {
      if let Some(entry) = notif.get(ModEntry::AUTO_UPDATE) {
        let widget = Flex::column()
          .with_child(Label::new(format!("Would you like to automatically update {}?", entry.name)))
          .with_child(Label::new(format!("Installed version: {}", entry.version)))
          .with_child(Label::new(format!("New version: {}", entry.remote_version.as_ref().and_then(|v| Some(v.version.to_string())).unwrap_or(String::from("Error: failed to retrieve version, this shouldn't be possible.")))))
          .with_default_spacer()
          .with_child(
            Flex::row()
              .with_child(Button::new("Update").on_click({
                let entry = entry.clone();
                move |ctx, _, _| {
                  ctx.submit_command(commands::CLOSE_WINDOW);
                  ctx.submit_command(ModList::AUTO_UPDATE.with(entry.clone()).to(Target::Global))
                }
              }))
              .with_child(Button::new("Cancel").on_click(|ctx, _, _| {
                ctx.submit_command(commands::CLOSE_WINDOW)
              }))
          ).cross_axis_alignment(druid::widget::CrossAxisAlignment::Start);

        ctx.new_sub_window(
          WindowConfig::default().resizable(true).window_size((500.0, 200.0)),
          widget,
          mod_list.clone(),
          env.clone()
        );
      }
    }

    child.event(ctx, event, mod_list, env)
  }
}

#[derive(Serialize, Deserialize)]
pub struct EnabledMods {
  #[serde(rename = "enabledMods")]
  enabled_mods: Vec<String>
}

impl EnabledMods {
  pub fn save(self, path: &PathBuf) -> Result<(), SaveError> {
    use std::fs;
    use std::io::Write;

    let json = serde_json::to_string_pretty(&self)
      .map_err(|_| SaveError::FormatError)?;

    let mut file = fs::File::create(path.join("mods").join("enabled_mods.json"))
      .map_err(|_| SaveError::FileError)?;

    file.write_all(json.as_bytes())
      .map_err(|_| SaveError::WriteError)
  }
}

impl From<Vec<Arc<ModEntry>>> for EnabledMods {
  fn from(from: Vec<Arc<ModEntry>>) -> Self {
    Self {
      enabled_mods: from.iter().into_iter().map(|v| v.id.clone()).collect()
    }
  }
}

#[derive(Debug, Clone, Data, PartialEq, Eq)]
pub enum ModEntryComp {
  ID,
  Name,
  Author,
  GameVersion,
  Enabled,
  Version,
  Score,
  AutoUpdateSupport
}

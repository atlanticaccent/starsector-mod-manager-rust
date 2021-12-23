use std::{
  io::{Read, BufReader, BufRead},
  path::PathBuf, collections::HashMap,
  fs::File,
  fmt::Display
};
use iced::{
  Text, Column, Command, Element, Length, Row, Scrollable, scrollable, Button,
  button, Checkbox, Container, Rule, PickList, pick_list, Space, Tooltip,
  tooltip, Subscription, TextInput, text_input, Align
};
use serde::{Serialize, Deserialize};
use json_comments::strip_comments;
use json5;
use handwritten_json;
use if_chain::if_chain;
use opener;
use sublime_fuzzy::best_match;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
  static ref VERSION_REGEX: Regex = Regex::new(r"\.|a-RC|A-RC|a-rc|a").unwrap();
}

use serde_aux::prelude::*;

use crate::gui::installer::{self, Installation};
use crate::style;
use crate::gui::SaveError;
use crate::gui::util;

mod headings;
use headings::{Headings, HeadingsMessage};

pub struct ModList {
  root_dir: Option<PathBuf>,
  pub mods: HashMap<String, ModEntry>,
  scroll: scrollable::State,
  pub mod_description: ModDescription,
  install_state: pick_list::State<InstallOptions>,
  tool_state: pick_list::State<ToolOptions>,
  currently_highlighted: Option<String>,
  sorting: (ModEntryComp, bool),
  name_id_ratio: f32,
  id_author_ratio: f32,
  author_version_ratio: f32,
  mod_version_auto_update_ratio: f32,
  auto_update_game_version_ratio: f32,
  pub last_browsed: Option<PathBuf>,
  headings: Headings,
  installs: Vec<Installation<u16>>,
  installation_id: u16,
  search_state: text_input::State,
  search_query: Option<String>,
  pub starsector_version: (Option<String>, Option<String>, Option<String>, Option<String>),
}

#[derive(Debug, Clone)]
pub enum ModListMessage {
  SetRoot(Option<PathBuf>),
  SetLastBrowsed(Option<PathBuf>),
  ModEntryMessage(String, ModEntryMessage),
  ModDescriptionMessage(ModDescriptionMessage),
  InstallPressed(InstallOptions),
  ToolsPressed(ToolOptions),
  EnabledModsSaved(Result<(), SaveError>),
  InstallationComplete(u16, Vec<String>, Vec<String>),
  DuplicateMod(String, String, installer::HybridPath, Option<PathBuf>),
  SingleInstallComplete,
  MasterVersionReceived((String, Result<ModVersionMeta, String>)),
  ParseModListError(()),
  HeadingsMessage(HeadingsMessage),
  SearchChanged(String),
  SetVersion(String),
}

impl ModList {
  pub fn new() -> Self {
    ModList {
      root_dir: None,
      mods: HashMap::new(),
      scroll: scrollable::State::new(),
      mod_description: ModDescription::new(),
      install_state: pick_list::State::default(),
      tool_state: pick_list::State::default(),
      currently_highlighted: None,
      sorting: (ModEntryComp::ID, false),
      name_id_ratio: Headings::NAME_ID_RATIO,
      id_author_ratio: Headings::ID_AUTHOR_RATIO,
      author_version_ratio: Headings::AUTHOR_MOD_VERSION_RATIO,
      mod_version_auto_update_ratio: Headings::MOD_VERSION_AUTO_UPDATE_RATIO,
      auto_update_game_version_ratio: Headings::AUTO_UPDATE_GAME_VERSION_RATIO,
      last_browsed: None,
      headings: Headings::new().unwrap(),
      installs: vec![],
      installation_id: 0,
      search_state: text_input::State::default(),
      search_query: None,
      starsector_version: (None, None, None, None),
    }
  }

  /**
   * Note: any branch that deals with mod installation, whether it be by replacement or whatever, _must_ call parse_mod_folder afterwards
   * Even if the result is an error, it's better to live with an increased computation cost in rare cases than it is to possibly miss a 
   * change in the state of the mods directory.
   */
  pub fn update(&mut self, message: ModListMessage) -> Command<ModListMessage> {
    match message {
      ModListMessage::SetVersion(version) => {
        self.starsector_version = parse_game_version(&version);

        Command::none()
      },
      ModListMessage::SetRoot(root_dir) => {
        if self.root_dir != root_dir {
          self.root_dir = root_dir;
  
          Command::batch(self.parse_mod_folder())
        } else {
          Command::none()
        }
      },
      ModListMessage::SetLastBrowsed(last_browsed) => {
        self.last_browsed = last_browsed;

        Command::none()
      }
      ModListMessage::ModEntryMessage(id, message) => {
        if let Some(entry) = self.mods.get_mut(&id) {
          match message {
            ModEntryMessage::EntryHighlighted => {
              self.mod_description.update(ModDescriptionMessage::ModChanged(entry.clone()));

              entry.update(ModEntryMessage::EntryHighlighted);

              if let Some(key) = &self.currently_highlighted {
                if !id.eq(key) {
                  let key = key.clone();
                  if let Some(old_entry) = self.mods.get_mut(&key) {
                    old_entry.update(ModEntryMessage::EntryCleared);
                  }
                }
              }

              self.currently_highlighted = Some(id);
            },
            ModEntryMessage::EntryCleared => {},
            ModEntryMessage::ToggleEnabled(_) => {
              entry.update(message);

              if let Some(path) = &self.root_dir {
                let enabled_mods = EnabledMods {
                  enabled_mods: self.mods.iter()
                    .filter_map(|(id, ModEntry { enabled, .. })| if *enabled {
                      Some(id.clone())
                    } else {
                      None 
                    })
                    .collect(),
                };
                return Command::perform(enabled_mods.save(path.join("mods").join("enabled_mods.json")), ModListMessage::EnabledModsSaved)
              }
            },
            ModEntryMessage::AutoUpdate => {
              self.update(ModListMessage::ModEntryMessage(id, ModEntryMessage::EntryHighlighted));
            }
          }
        }

        Command::none()
      },
      ModListMessage::ModDescriptionMessage(message) => {
        self.mod_description.update(message);

        Command::none()
      },
      ModListMessage::InstallPressed(opt) => {
        if let Some(root_dir) = self.root_dir.clone() {
          let start_path = if let Some(last_browsed) = &self.last_browsed {
            &last_browsed
          } else {
            &root_dir
          }.to_str().expect("Convert path to string");

          match opt {
            InstallOptions::FromSingleArchive | InstallOptions::FromMultipleArchive => {
              if let Some(paths) = util::select_archives(start_path) {
                if let Some(last) = paths.last() {
                  self.last_browsed = PathBuf::from(last).parent().map(|p| p.to_path_buf());
                }

                let mod_ids: Vec<String> = self.mods.iter().map(|(id, _)| id.clone()).collect();
                self.installs.push(Installation::new(
                  self.installation_id,
                  paths,
                  root_dir.join("mods"),
                  mod_ids
                ));

                self.installation_id += 1;
              }
              Command::none()
            },
            InstallOptions::FromFolder => {
              match util::select_folder_dialog("Select mod folder:", start_path) {
                Some(source_path) => {
                  self.last_browsed = PathBuf::from(&source_path).parent().map(|p| p.to_path_buf());
                  
                  let mod_ids: Vec<String> = self.mods.iter().map(|(id, _)| id.clone()).collect();
                  self.installs.push(Installation::new(
                    self.installation_id,
                    vec![PathBuf::from(source_path)],
                    root_dir.join("mods"),
                    mod_ids
                  ));

                  self.installation_id += 1;
                },
                None => {},
              }

              Command::none()
            },
            InstallOptions::FromDownload(url, target_version, old_path) => {
              self.installs.push(Installation::new(
                self.installation_id,
                (url, target_version, old_path),
                root_dir.join("mods"),
                Vec::new()
              ));

              self.installation_id += 1;

              Command::none()
            },
            _ => Command::none()
          }
        } else {
          util::error("No install directory set. Please set the Starsector install directory in Settings.");
          return Command::none();
        }
      },
      ModListMessage::EnabledModsSaved(res) => {
        match res {
          // Err(err) => debug_println!("{:?}", err),
          _ => {}
        }

        Command::none()
      },
      ModListMessage::InstallationComplete(id, successful, failed) => {
        self.installs.retain(|i| i.id != id);

        let complete = if successful.len() > 0 {
          format!("Succesfully installed:\n{}\n", successful.join(", "))
        } else {
          String::new()
        };
        let errors = if failed.len() > 0 {
          format!("Failed to install:\n{}", failed.join(", "))
        } else {
          String::new()
        };
        if successful.len() > 0 || failed.len() > 0 {
          util::notif(format!("{}{}", complete, errors));
        }

        Command::batch(self.parse_mod_folder())
      },
      ModListMessage::DuplicateMod(name, id, new_path, old_path) => {
        if let Some(old_path) = old_path {
          let id = if let Some(mod_id) = self.mods.iter().find_map(|(_, entry)| (entry.path == old_path).then(|| entry.id.clone())) {
            format!(", containing mod with ID `{}`, ", mod_id)
          } else {
            String::new()
          };
          let folder_name = old_path.file_name().unwrap().to_string_lossy();
          if util::query(format!("A folder named `{}`{} already exists. Do you want to replace it?\nClicking no will cancel the installation of this mod.", folder_name, id)) {
            self.installs.push(Installation::new(
              self.installation_id,
              (name, new_path, old_path),
              PathBuf::new(),
              vec![]
            ));

            self.installation_id += 1;
          };
        } else {
          if let Some((_, entry)) = self.mods.iter().find(|(_, entry)| entry.id == id) {
            if util::query(format!("A mod with ID `{}`, named `{}`, already exists. Do you want to replace it?\nClicking no will cancel the installation of this mod.", id, name)) {
              self.installs.push(Installation::new(
                self.installation_id,
                (name, new_path, entry.path.clone()),
                self.root_dir.clone().unwrap(),
                vec![]
              ));

              self.installation_id += 1;
            };
          }
        }

        Command::none()
      },
      ModListMessage::SingleInstallComplete => {
        Command::batch(self.parse_mod_folder())
      }
      ModListMessage::MasterVersionReceived((id, res)) => {
        if_chain! {
          if let Some(entry) = self.mods.get_mut(&id);
          if let Some(ModVersionMeta { version: local_version, .. }) = &entry.version_checker;
          then {
            match res {
              Ok(remote_version_meta) => {
                let version = remote_version_meta.version.clone();

                if version == *local_version {
                  entry.update_status = Some(UpdateStatus::UpToDate)
                } else if version < *local_version {
                  entry.update_status = Some(UpdateStatus::Discrepancy(version))
                } else if version.major - local_version.major > 0 {
                  entry.update_status = Some(UpdateStatus::Major(version))
                } else if version.minor - local_version.minor > 0 {
                  entry.update_status = Some(UpdateStatus::Minor(version))
                } else {
                  entry.update_status = Some(UpdateStatus::Patch(version))
                };
                entry.remote_version = Some(remote_version_meta);
              },
              Err(_err) => {
                // debug_println!("Could not get remote update data for {}.\nError: {}", id, err);
                entry.update_status = Some(UpdateStatus::Error)
              }
            }
          } else {
            dbg!("Have a remote version file, but either local entry or local version file are missing, which is odd to say the least.");
          }
        };

        Command::none()
      },
      ModListMessage::ParseModListError(_) => {
        util::error(format!("Failed to parse mods folder. Mod list has not been populated."));

        Command::none()
      },
      ModListMessage::ToolsPressed(opt) => {
        match opt {
          ToolOptions::Default => { Command::none() },
          ToolOptions::EnableAll => {
            if let Some(path) = &self.root_dir {
              let mut enabled_mods: Vec<String> = vec![];
              self.mods.iter_mut()
                .for_each(|(id, entry)| {
                  enabled_mods.push(id.clone());
                  entry.update(ModEntryMessage::ToggleEnabled(true));
                });

              Command::perform(EnabledMods { enabled_mods }.save(path.join("mods").join("enabled_mods.json")), ModListMessage::EnabledModsSaved)
            } else {
              Command::none()
            }
          },
          ToolOptions::DisableAll => {
            if let Some(path) = &self.root_dir {
              self.mods.iter_mut()
                .for_each(|(_, entry)| {
                  entry.update(ModEntryMessage::ToggleEnabled(false));
                });

              Command::perform(EnabledMods { enabled_mods: vec![] }.save(path.join("mods").join("enabled_mods.json")), ModListMessage::EnabledModsSaved)
            } else {
              Command::none()
            }
          },
          ToolOptions::FilterDisabled => {
            self.mods.iter_mut()
              .for_each(|(_, entry)| {
                entry.display = !entry.enabled;
              });

            Command::none()
          },
          ToolOptions::FilterEnabled => {
            self.mods.iter_mut()
              .for_each(|(_, entry)| {
                entry.display = entry.enabled;
              });

            Command::none()
          },
          ToolOptions::FilterOutdated => {
            self.mods.iter_mut()
              .for_each(|(_, entry)| {
                entry.display = matches!(entry.update_status, Some(UpdateStatus::Major(_)) | Some(UpdateStatus::Minor(_)) | Some(UpdateStatus::Patch(_)));
              });

            Command::none()
          },
          ToolOptions::FilterError => {
            self.mods.iter_mut()
              .for_each(|(_, entry)| {
                entry.display = matches!(entry.update_status, Some(UpdateStatus::Error));
              });

            Command::none()
          },
          ToolOptions::FilterUnsupported => {
            self.mods.iter_mut()
              .for_each(|(_, entry)| {
                entry.display = matches!(entry.update_status, None);
              });

            Command::none()
          },
          ToolOptions::FilterDiscrepancy => {
            self.mods.iter_mut()
              .for_each(|(_, entry)| {
                entry.display = matches!(entry.update_status, Some(UpdateStatus::Discrepancy(_)));
              });

            Command::none()
          }
          ToolOptions::FilterNone => {
            self.mods.iter_mut()
              .for_each(|(_, entry)| {
                entry.display = true;
              });

            Command::none()
          },
          ToolOptions::Refresh => {
            Command::batch(self.parse_mod_folder())
          }
        }
      },
      ModListMessage::HeadingsMessage(message) => {
        match message {
          HeadingsMessage::HeadingPressed(sorting) => {
            let (current, val) = &self.sorting;
            if *current == sorting {
              self.sorting = (sorting, !val)
            } else {
              self.sorting = (sorting, false)
            }
          },
          HeadingsMessage::Resized(event) => {
            if event.split == self.headings.name_id_split {
              self.name_id_ratio = event.ratio;
            } else if event.split == self.headings.id_author_split {
              self.id_author_ratio = event.ratio;
            } else if event.split == self.headings.author_mod_version_split {
              self.author_version_ratio = event.ratio;
            } else if event.split == self.headings.mod_version_auto_update_split {
              self.mod_version_auto_update_ratio = if event.ratio > Headings::MOD_VERSION_AUTO_UPDATE_RATIO {
                Headings::MOD_VERSION_AUTO_UPDATE_RATIO
              } else {
                event.ratio
              };
            } else if event.split == self.headings.auto_update_game_version_split {
              self.auto_update_game_version_ratio = if event.ratio < Headings::AUTO_UPDATE_GAME_VERSION_RATIO {
                Headings::AUTO_UPDATE_GAME_VERSION_RATIO
              } else {
                event.ratio
              };
            }

            self.headings.update(message);
          }
        }

        Command::none()
      },
      ModListMessage::SearchChanged(search_query) => {
        self.search_query = Some(search_query.clone());

        if search_query.len() > 0 {
          self.mods.iter_mut().for_each(|(id, mod_entry)| {
            let id_score = best_match(&search_query, id).map(|m| m.score());
            let name_score = best_match(&search_query, &mod_entry.name).map(|m| m.score());
            let author_score = best_match(&search_query, &mod_entry.author).map(|m| m.score());

            mod_entry.display = id_score.is_some() || name_score.is_some() || author_score.is_some();
            mod_entry.search_score = std::cmp::max(std::cmp::max(id_score, name_score), author_score);
          });

          self.sorting = (ModEntryComp::Score, true);
        } else {
          self.mods.iter_mut().for_each(|(_, mod_entry)| {
            mod_entry.display = true;
          });
          
          self.sorting = (ModEntryComp::ID, false);
        }

        Command::none()
      }
    }
  }

  pub fn view(&mut self) -> Element<ModListMessage> {
    let starsector_version = self.starsector_version.clone();
    let mut every_other = true;
    let content = Column::new()
      .push(Row::new()
        .push(PickList::new(
          &mut self.install_state,
          &InstallOptions::SHOW[..],
          Some(InstallOptions::Default),
          ModListMessage::InstallPressed
        ))
        .push(PickList::new(
          &mut self.tool_state,
          &ToolOptions::SHOW[..],
          Some(ToolOptions::Default),
          ModListMessage::ToolsPressed
        ))
        .push(Space::with_width(Length::Units(5)))
        .push(Container::new(Text::new("Search:").height(Length::Fill)).padding(5))
        .push(TextInput::new(
          &mut self.search_state,
          "",
          if let Some(ref query) = self.search_query {
            query
          } else { "" },
          ModListMessage::SearchChanged
        ).padding(5))
      )
      .push(Space::with_height(Length::Units(10)))
      .push(Column::new()
        .push(Row::new()
          .push(self.headings.view().map(|message| {
            ModListMessage::HeadingsMessage(message)
          }))
          .push(Space::with_width(Length::Units(10)))
          .height(Length::Shrink)
        )
      )
      .push(Rule::horizontal(2).style(style::max_rule::Rule))
      .push(Scrollable::new(&mut self.scroll)
        .height(Length::FillPortion(3))
        .push(Row::new()
          .push::<Element<ModListMessage>>(if self.mods.len() > 0 {
            let mut sorted_mods = self.mods
              .iter_mut()
              .map(|(_, entry)| entry)
              .collect::<Vec<&mut ModEntry>>();

            let cmp = &self.sorting;
            sorted_mods.sort_by(|left, right| {
              match cmp {
                (ModEntryComp::Score, _) => right.search_score.cmp(&left.search_score),
                (ModEntryComp::ID, false) => left.id.cmp(&right.id),
                (ModEntryComp::Name, false) => left.name.cmp(&right.name),
                (ModEntryComp::Author, false) => left.author.cmp(&right.author),
                (ModEntryComp::Enabled, false) => left.enabled.cmp(&right.enabled),
                (ModEntryComp::GameVersion, false) => left.game_version.cmp(&right.game_version),
                (ModEntryComp::Version, false) => {
                  if left.update_status.is_none() && right.update_status.is_none() {
                    left.name.cmp(&right.name)
                  } else if left.update_status.is_none() {
                    std::cmp::Ordering::Greater
                  } else if right.update_status.is_none() {
                    std::cmp::Ordering::Less
                  } else {
                    if left.update_status.cmp(&right.update_status) == std::cmp::Ordering::Equal {
                      left.name.cmp(&right.name)
                    } else {
                      left.update_status.cmp(&right.update_status)
                    }
                  }

                },
                (ModEntryComp::AutoUpdateSupport, true) => {
                  left.remote_version.as_ref().and_then(|r| r.direct_download_url.as_ref()).is_some()
                    .cmp(&right.remote_version.as_ref().and_then(|r| r.direct_download_url.as_ref()).is_some())
                }
                (ModEntryComp::ID, true) => right.id.cmp(&left.id),
                (ModEntryComp::Name, true) => right.name.cmp(&left.name),
                (ModEntryComp::Author, true) => right.author.cmp(&left.author),
                (ModEntryComp::Enabled, true) => right.enabled.cmp(&left.enabled),
                (ModEntryComp::GameVersion, true) => right.game_version.cmp(&left.game_version),
                (ModEntryComp::Version, true) => {
                  if right.update_status.is_none() && left.update_status.is_none() {
                    left.name.cmp(&right.name)
                  } else if right.update_status.is_none() {
                    std::cmp::Ordering::Greater
                  } else if left.update_status.is_none() {
                    std::cmp::Ordering::Less
                  } else if right.update_status.cmp(&left.update_status) == std::cmp::Ordering::Equal {
                    left.name.cmp(&right.name)
                  } else {
                    right.update_status.cmp(&left.update_status)
                  }
                },
                (ModEntryComp::AutoUpdateSupport, false) => {
                  right.remote_version.as_ref().and_then(|r| r.direct_download_url.as_ref()).is_some()
                    .cmp(&left.remote_version.as_ref().and_then(|r| r.direct_download_url.as_ref()).is_some())
                }
              }
            });

            let mut views: Vec<Element<ModListMessage>> = vec![];
            let name_portion = 10000.0 * self.name_id_ratio;
            let id_portion = (10000.0 - name_portion) * self.id_author_ratio;
            let author_portion = (10000.0 - name_portion - id_portion) * self.author_version_ratio;
            let mod_version_portion = (10000.0 - name_portion - id_portion - author_portion) * self.mod_version_auto_update_ratio;
            let auto_update_portion = (10000.0 - name_portion - id_portion - author_portion - mod_version_portion) * self.auto_update_game_version_ratio;
            let game_version_portion = 10000.0 - name_portion - id_portion - author_portion - mod_version_portion - auto_update_portion;

            sorted_mods.into_iter()
              .filter(|entry| entry.display)
              .for_each(|entry| {
                every_other = !every_other;
                let id_clone = entry.id.clone();
                views.push(entry.view(every_other,
                  name_portion as u16,
                  id_portion as u16,
                  author_portion as u16,
                  mod_version_portion as u16,
                  auto_update_portion as u16,
                  game_version_portion as u16,
                  starsector_version.clone()
                ).map(move |message| {
                  ModListMessage::ModEntryMessage(id_clone.clone(), message)
                }))
              });

            Column::with_children(views).into()
          } else {
            Column::new()
              .width(Length::Fill)
              .height(Length::Units(200))
              .push(Text::new("No mods found") //change this to be more helpful
                .width(Length::Fill)
                .size(25)
                .color([0.7, 0.7, 0.7])
              )
              .into()
          })
          .push(Space::with_width(Length::Units(10)))
        )
      )
      .push(Rule::horizontal(1).style(style::max_rule::Rule))
      .push(Space::with_height(Length::Units(5)))
      .push(
        Container::new(self.mod_description.view().map(|message| {
          ModListMessage::ModDescriptionMessage(message)
        }))
        .height(Length::FillPortion(2))
        .width(Length::Fill)
      );

    Column::new()
      .push(content)
      .padding(5)
      .height(Length::Fill)
      .into()
  }

  pub fn subscription(&self) -> Subscription<ModListMessage> {
    if self.installs.len() > 0 {
      return Subscription::batch(self.installs.iter().map(|i| i.clone().install())).map(|message| match message {
        installer::Progress::Query(name, id, new_path, old_path) => ModListMessage::DuplicateMod(name, id, new_path, old_path),
        installer::Progress::Completed(id, completed, failed) => ModListMessage::InstallationComplete(id, completed, failed),
        installer::Progress::Finished => ModListMessage::SingleInstallComplete
      })
    }

    Subscription::none()
  }

  #[must_use]
  fn parse_mod_folder(&mut self) -> Vec<Command<ModListMessage>>{
    self.mods.clear();

    if let Some(root_dir) = &self.root_dir {
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
            return vec![Command::perform(async {}, ModListMessage::ParseModListError)]
          }
        }
      };

      if let Ok(dir_iter) = std::fs::read_dir(mod_dir) {
        let enabled_mods_iter = enabled_mods.iter();

        let (mods, versions): (Vec<(String, ModEntry)>, Vec<Option<ModVersionMeta>>) = dir_iter
          .filter_map(|entry| entry.ok())
          .filter(|entry| {
            if let Ok(file_type) = entry.file_type() {
              file_type.is_dir()
            } else {
              false
            }
          })
          .filter_map(|entry| {
            if let Ok(mut mod_info) = ModEntry::from_file(entry.path().join("mod_info.json")) {
              let version = if_chain! {
                if let Ok(version_loc_file) = File::open(entry.path().join("data").join("config").join("version").join("version_files.csv"));
                let lines = BufReader::new(version_loc_file).lines();
                if let Some(Ok(version_filename)) = lines.skip(1).next();
                if let Ok(version_data) = std::fs::read_to_string(entry.path().join(version_filename));
                let mut no_comments = String::new();
                if strip_comments(version_data.as_bytes()).read_to_string(&mut no_comments).is_ok();
                if let Ok(normalized) = handwritten_json::normalize(&no_comments);
                if let Ok(mut version) = json5::from_str::<ModVersionMeta>(&normalized);
                then {
                  version.id = mod_info.id.clone();
                  Some(version)
                } else {
                  None
                }
              };
              mod_info.enabled = enabled_mods_iter.clone().find(|id| mod_info.id.clone().eq(*id)).is_some();
              mod_info.version_checker = version.clone();
              Some((
                (
                  mod_info.id.clone(),
                  mod_info.clone()
                ),
                version
              ))
            } else {
              dbg!(entry.path());
              None
            }
          })
          .unzip();

        self.mods.extend(mods);

        versions.iter()
          .filter_map(|v| v.as_ref())
          .map(|v| Command::perform(util::get_master_version(v.clone()), ModListMessage::MasterVersionReceived))
          .collect()
      } else {
        // debug_println!("Fatal. Could not parse mods folder. Alert developer");
        vec![Command::perform(async {}, ModListMessage::ParseModListError)]
      }
    } else {
      vec![Command::perform(async {}, ModListMessage::ParseModListError)]
    }
  }

  pub fn get_game_version(&self) -> Option<String> {
    match &self.starsector_version {
      (None, None, None, None) => None,
      (major, minor, patch, rc) => {
        Some(format!(
          "{}.{}{}{}",
          major.clone().unwrap_or("0".to_string()),
          minor.clone().unwrap_or("".to_string()),
          patch.clone().map_or_else(|| "".to_string(), |p| format!(".{}", p)),
          rc.clone().map_or_else(|| "".to_string(), |rc| format!("a-RC{}", rc))
        ))
      }
    }
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InstallOptions {
  FromMultipleArchive,
  FromSingleArchive,
  FromFolder,
  FromDownload(String, String, PathBuf),
  Default
}

impl InstallOptions {
  const SHOW: [InstallOptions; 3] = [
    InstallOptions::FromMultipleArchive,
    InstallOptions::FromSingleArchive,
    InstallOptions::FromFolder
  ];
}

impl std::fmt::Display for InstallOptions {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}",
      match self {
        InstallOptions::Default => "Install Mod",
        InstallOptions::FromMultipleArchive => "From Multiple Archives",
        InstallOptions::FromSingleArchive => "From Single Archive",
        InstallOptions::FromDownload(_, _, _) => "From Download",
        InstallOptions::FromFolder => "From Folder"
      }
    )
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ToolOptions {
  Default,
  EnableAll,
  DisableAll,
  FilterEnabled,
  FilterDisabled,
  FilterOutdated,
  FilterError,
  FilterUnsupported,
  FilterDiscrepancy,
  FilterNone,
  Refresh,
}

impl ToolOptions {
  const SHOW: [ToolOptions; 10] = [
    ToolOptions::EnableAll,
    ToolOptions::DisableAll,
    ToolOptions::FilterEnabled,
    ToolOptions::FilterDisabled,
    ToolOptions::FilterOutdated,
    ToolOptions::FilterError,
    ToolOptions::FilterUnsupported,
    ToolOptions::FilterDiscrepancy,
    ToolOptions::FilterNone,
    ToolOptions::Refresh,
  ];
}

impl std::fmt::Display for ToolOptions {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}",
      match self {
        ToolOptions::Default => "Tools",
        ToolOptions::EnableAll => "Enable All",
        ToolOptions::DisableAll => "Disable All",
        ToolOptions::FilterEnabled => "Show Enabled",
        ToolOptions::FilterDisabled => "Show Disabled",
        ToolOptions::FilterOutdated => "Show New Version Available",
        ToolOptions::FilterError => "Show Version Check Failed",
        ToolOptions::FilterUnsupported => "Show Version Check Unsupported",
        ToolOptions::FilterDiscrepancy => "Show Version Discrepancy",
        ToolOptions::FilterNone => "Show All",
        ToolOptions::Refresh => "Refresh Mod List",
      }
    )
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpdateStatus {
  Error,
  Major(Version),
  Minor(Version),
  Patch(Version),
  UpToDate,
  Discrepancy(Version),
}

impl Display for UpdateStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    match self {
      UpdateStatus::Major(_) => write!(f, "Major"),
      UpdateStatus::Minor(_) => write!(f, "Minor"),
      UpdateStatus::Patch(_) => write!(f, "Patch"),
      UpdateStatus::UpToDate => write!(f, "Up to date"),
      UpdateStatus::Error => write!(f, "Error"),
      UpdateStatus::Discrepancy(_) => write!(f, "Discrepancy"),
    }
  }
}

pub struct UpdateStatusTTPatch(pub UpdateStatus);

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum VersionUnion {
  String(String),
  Object(Version)
}

impl Display for VersionUnion {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    let output: String = match self {
      VersionUnion::String(s) => s.to_string(),
      VersionUnion::Object(o) => o.to_string()
    };
    write!(f, "{}", output)
  }
}

impl From<VersionUnion> for String {
  fn from(version_union: VersionUnion) -> Self {
    version_union.to_string()
  }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModEntry {
  pub id: String,
  pub name: String,
  #[serde(default)]
  author: String,
  pub version: VersionUnion,
  description: String,
  #[serde(alias = "gameVersion")]
  game_version: String,
  #[serde(skip)]
  parsed_game_version: (Option<String>, Option<String>, Option<String>, Option<String>),
  #[serde(skip)]
  enabled: bool,
  #[serde(skip)]
  highlighted: bool,
  #[serde(skip)]
  version_checker: Option<ModVersionMeta>,
  #[serde(skip)]
  pub remote_version: Option<ModVersionMeta>,
  #[serde(skip)]
  update_status: Option<UpdateStatus>,
  #[serde(skip)]
  pub path: PathBuf,
  #[serde(skip)]
  #[serde(default = "button::State::new")]
  button_state: button::State,
  #[serde(skip)]
  #[serde(default = "button::State::new")]
  auto_update_button_state: button::State,
  #[serde(skip)]
  #[serde(default = "ModEntry::def_true")]
  display: bool,
  #[serde(skip)]
  search_score: Option<isize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub enum ModEntryMessage {
  ToggleEnabled(bool),
  EntryHighlighted,
  EntryCleared,
  AutoUpdate
}

pub enum ModEntryError {
  ParseError,
  FileError
}

impl ModEntry {
  const ICONS: iced::Font = iced::Font::External {
    name: "Icons",
    bytes: std::include_bytes!("../../assets/icons.ttf")
  };

  pub fn from_file(mut path: PathBuf) -> Result<ModEntry, ModEntryError> {
    if let Ok(mod_info_file) = std::fs::read_to_string(path.clone()) {
      if_chain! {
        let mut stripped = String::new();
        if strip_comments(mod_info_file.as_bytes()).read_to_string(&mut stripped).is_ok();
        if let Ok(mut mod_info) = json5::from_str::<ModEntry>(&stripped);
        then {
          path.pop();
          mod_info.path = path;
          mod_info.parsed_game_version = parse_game_version(&mod_info.game_version);
          Ok(mod_info)
        } else {
          Err(ModEntryError::ParseError)
        }
      }
    } else {
      Err(ModEntryError::FileError)
    }
  }

  pub fn update(&mut self, message: ModEntryMessage) -> Command<ModEntryMessage> {
    match message {
      ModEntryMessage::ToggleEnabled(enabled) => {
        self.enabled = enabled;

        Command::none()
      },
      ModEntryMessage::EntryHighlighted => {
        self.highlighted = true;

        Command::none()
      },
      ModEntryMessage::EntryCleared => {
        self.highlighted = false;

        Command::none()
      },
      ModEntryMessage::AutoUpdate => {
        Command::none()
      }
    }
  }

  pub fn view(
    &mut self,
    other: bool,
    name_portion: u16,
    id_portion: u16,
    author_portion: u16,
    mod_version_portion: u16,
    auto_update_portion: u16,
    game_version_portion: u16,
    starsector_version: (Option<std::string::String>, Option<std::string::String>, Option<std::string::String>, Option<std::string::String>)
  ) -> Element<ModEntryMessage> {
    let auto_update_supported = self.remote_version.as_ref().and_then(|remote| remote.direct_download_url.as_ref()).is_some();

    let mut auto_update_button = Button::new(
      &mut self.auto_update_button_state,
      Row::new()
        .push(Rule::vertical(0).style(style::max_rule::Rule))
        .push(Column::with_children(vec![
          Space::with_height(Length::FillPortion(1)).into(),
          {
            let text = Text::new(if auto_update_supported {
              '\u{f270}'.to_string()
            } else {
              '\u{f623}'.to_string()
            }).font(ModEntry::ICONS).size(32);

            if !auto_update_supported {
              text.color(iced::Color::from_rgb8(0xB0, 0x00, 0x20))
            } else {
              text
            }.into()
          },
          Space::with_height(Length::FillPortion(1)).into(),
        ]).width(Length::Fill).height(Length::Fill).align_items(Align::Center))
    )
    .width(Length::FillPortion(auto_update_portion))
    .height(Length::Fill)
    .padding(0);

    auto_update_button = if auto_update_supported {
      let button = auto_update_button.style(style::button_highlight_and_hover_green::Button);

      if self.update_status != Some(UpdateStatus::UpToDate) {
        button.on_press(ModEntryMessage::AutoUpdate)
      } else {
        button
      }
    } else {
      auto_update_button.style(style::button_none::Button)
    };

    let row = Container::new(Row::new()
      .push(
        Container::new(
          Checkbox::new(self.enabled, "", move |toggled| {
            ModEntryMessage::ToggleEnabled(toggled)
          })
        )
        .center_x()
        .center_y()
        .width(Length::FillPortion(Headings::ENABLED_PORTION as u16))
        .height(Length::Fill)
      )
      .push(
        Button::new(
          &mut self.button_state,
          Row::new()
            .push(Container::new(Row::new()
              .push(Rule::vertical(0).style(style::max_rule::Rule))
              .push(Space::with_width(Length::Units(5)))
              .push(Text::new(self.name.clone()).width(Length::Fill))
            ).width(Length::FillPortion(name_portion)))
            .push(Container::new(Row::new()
              .push(Rule::vertical(0).style(style::max_rule::Rule))
              .push(Space::with_width(Length::Units(5)))
              .push(Text::new(self.id.clone()).width(Length::Fill))
            ).width(Length::FillPortion(id_portion)))
            .push(Container::new(Row::new()
              .push(Rule::vertical(0).style(style::max_rule::Rule))
              .push(Space::with_width(Length::Units(5)))
              .push(Text::new(self.author.clone()).width(Length::Fill))
            ).width(Length::FillPortion(author_portion)))
            .push::<Element<ModEntryMessage>>(
              if let Some(status) = &self.update_status {
                Container::new(
                  Tooltip::new(
                    Container::new(Row::with_children(vec![
                      Rule::vertical(0).style(style::max_rule::Rule).into(),
                      Space::with_width(Length::Units(5)).into(),
                      if let UpdateStatus::Major(remote) | UpdateStatus::Minor(remote) | UpdateStatus::Patch(remote) = status {
                        Column::with_children(vec![
                          Space::with_height(Length::Units(5)).into(),
                          Row::with_children(vec![
                            Text::new(format!("Installed:")).into(),
                            Space::with_width(Length::Fill).into(),
                            Text::new(self.version.to_string()).into(),
                          ]).width(Length::Fill).into(),
                          Space::with_height(Length::Fill).into(),
                          Row::with_children(vec![
                            Text::new(format!("Available:")).into(),
                            Space::with_width(Length::Fill).into(),
                            Text::new(remote.to_string()).into(),
                          ]).width(Length::Fill).into(),
                          Space::with_height(Length::Units(5)).into(),
                        ]).height(Length::Fill).width(Length::Fill).into()
                      } else {
                        Text::new(self.version.to_string()).into()
                      },
                      Space::with_width(Length::Units(5)).into(),
                    ]))
                    .style(status)
                    .width(Length::Fill)
                    .height(Length::Fill),
                    match status {
                      UpdateStatus::Major(_) | UpdateStatus::Minor(_) => {
                        format!("{} update available", status)
                      },
                      UpdateStatus::Patch(_) => format!("Patch available"),
                      UpdateStatus::UpToDate => format!("Up to date!"),
                      UpdateStatus::Error => format!("Could not retrieve remote update data"),
                      UpdateStatus::Discrepancy(_) => format!("Local is a higher version than remote")
                    },
                    tooltip::Position::FollowCursor
                  ).style(UpdateStatusTTPatch(status.clone()))
                )
                .width(Length::FillPortion(mod_version_portion))
                .height(Length::Fill)
                .padding(1)
                .into()
              } else {
                Container::new(Row::new()
                  .push(Rule::vertical(0).style(style::max_rule::Rule))
                  .push(Space::with_width(Length::Units(5)))
                  .push(Text::new(self.version.clone()).width(Length::Fill))
                ).width(Length::FillPortion(mod_version_portion))
                .into()
              }
            )
            .push(auto_update_button)
            .push(Container::new::<Element<ModEntryMessage>>({
                let game_version: Container<ModEntryMessage> = Container::new(Row::new()
                  .push(Rule::vertical(0).style(style::max_rule::Rule))
                  .push(Space::with_width(Length::Units(5)))
                  .push(Text::new(self.game_version.clone()).width(Length::Fill)))
                  .width(Length::Fill)
                  .height(Length::Fill);

                match (self.parsed_game_version.clone(), starsector_version) {
                  ((mod_major, ..), (game_major, ..)) if mod_major != game_major => {
                    Tooltip::new(
                      game_version.style(style::update::error::Container),
                      "Major version mismatch!\nAlmost guaranteed to crash!",
                      tooltip::Position::FollowCursor
                    ).style(style::update::error::Tooltip).into()
                  },
                  ((_, mod_minor, ..), (_, game_minor, ..)) if mod_minor != game_minor => {
                    Tooltip::new(
                      game_version.style(style::update::error::Container),
                      "Minor version mismatch!\nHighly likely to crash!",
                      tooltip::Position::FollowCursor
                    ).style(style::update::error::Tooltip).into()
                  },
                  ((.., mod_patch, _), (.., game_patch, _)) if mod_patch != game_patch => {
                    Tooltip::new(
                      game_version.style(style::update::major::Container),
                      "Patch version mismatch!\nMild possibility of issues.",
                      tooltip::Position::FollowCursor
                    ).style(style::update::major::Tooltip).into()
                  },
                  ((.., mod_rc), (.., game_rc)) if mod_rc != game_rc => {
                    Tooltip::new(
                      game_version.style(style::update::major::Container),
                      "Release Candidate mismatch.\nUnlikely to cause issues.",
                      tooltip::Position::FollowCursor
                    ).style(style::update::major::Tooltip).into()
                  },
                  ((.., mod_rc), (.., game_rc)) if mod_rc == game_rc => {
                    Tooltip::new(
                      game_version.style(style::update::up_to_date::Container),
                      "Up to date!",
                      tooltip::Position::FollowCursor
                    ).style(style::update::up_to_date::Tooltip).into()
                  }
                  _ => {
                    game_version.into()
                  }
                }
              }
            )
            .padding(1)
            .width(Length::FillPortion(game_version_portion)))
            .height(Length::Fill)
        )
        .padding(0)
        .height(Length::Fill)
        .style(style::button_none::Button)
        .on_press(ModEntryMessage::EntryHighlighted)
        .width(Length::FillPortion(Headings::REMAINING_PORTION as u16))
      )
      .height(Length::Units(50))
    );

    Row::new()
      .push(
        if self.highlighted {
          row.style(style::highlight_background::Container)
        } else if other {
          row.style(style::alternate_background::Container)
        } else {
          row
        }.width(Length::Fill)
      )
      .push(Space::with_width(Length::Units(10)))
      .into()
  }

  fn def_true() -> bool { true }

  pub fn get_master_version(&self) -> Option<&ModVersionMeta> {
    self.remote_version.as_ref()
  }
}

  /**
   * Parses a given version into a four-tuple of the assumed components.
   * Assumptions:
   * - The first component is always EITHER 0 and thus the major component OR it has been omitted and the first component is the minor component
   * - If there are two components it is either the major and minor components OR minor and patch OR minor and RC (release candidate)
   * - If there are three components it is either the major, minor and patch OR major, minor and RC OR minor, patch and RC
   * - If there are four components then the first components MUST be 0 and MUST be the major component, and the following components 
        are the minor, patch and RC components
   */
fn parse_game_version(text: &str) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
  let components: Vec<&str> = VERSION_REGEX.split(text).filter(|c| !c.is_empty()).collect();

  match components.as_slice() {
    [major, minor] if major == &"0" => {
      // text = format!("{}.{}a", major, minor);
      (Some(major.to_string()), Some(minor.to_string()), None, None)
    }
    [minor, patch_rc] => {
      // text = format!("0.{}a-RC{}", minor, rc);
      if text.contains("a-RC") {
        (Some("0".to_string()), Some(minor.to_string()), None, Some(patch_rc.to_string()))
      } else {
        (Some("0".to_string()), Some(minor.to_string()), Some(patch_rc.to_string()), None)
      }
    }
    [major, minor, patch_rc] if major == &"0" => {
      // text = format!("{}.{}a-RC{}", major, minor, rc);
      if text.contains("a-RC") {
        (Some(major.to_string()), Some(minor.to_string()), None, Some(patch_rc.to_string()))
      } else {
        (Some(major.to_string()), Some(minor.to_string()), Some(patch_rc.to_string()), None)
      }
    }
    [minor, patch, rc] => {
      // text = format!("0.{}.{}a-RC{}", minor, patch, rc);
      (Some("0".to_string()), Some(minor.to_string()), Some(patch.to_string()), Some(rc.to_string()))
    }
    [major, minor, patch, rc] if major == &"0" => {
      // text = format!("{}.{}.{}a-RC{}", major, minor, patch, rc);
      (Some(major.to_string()), Some(minor.to_string()), Some(patch.to_string()), Some(rc.to_string()))
    }
    _ => {
      dbg!("Failed to normalise mod's quoted game version");
      (None, None, None, None)
    }
  }
}

#[derive(Debug, Clone, Deserialize, Eq, Ord)]
pub struct ModVersionMeta {
  #[serde(alias="masterVersionFile")]
  pub remote_url: String,
  #[serde(alias="directDownloadURL")]
  #[serde(default)]
  pub direct_download_url: Option<String>,
  #[serde(alias="modName")]
  pub id: String,
  #[serde(alias="modThreadId")]
  #[serde(deserialize_with="deserialize_string_from_number")]
  #[serde(default)]
  fractal_id: String,
  #[serde(alias="modNexusId")]
  #[serde(deserialize_with="deserialize_string_from_number")]
  #[serde(default)]
  nexus_id: String,
  #[serde(alias="modVersion")]
  pub version: Version
}

impl PartialEq for ModVersionMeta {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.version == other.version
  }
}

impl PartialOrd for ModVersionMeta {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    self.version.partial_cmp(&other.version)
  }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
  #[serde(deserialize_with="deserialize_number_from_string")]
  pub major: i32,
  #[serde(deserialize_with="deserialize_number_from_string")]
  pub minor: i32,
  #[serde(default)]
  #[serde(deserialize_with="deserialize_string_from_number")]
  pub patch: String
}

impl Display for Version {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    if self.patch.len() > 0 {
      write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    } else {
      write!(f, "{}.{}", self.major, self.minor)
    }
  }
}

#[derive(Debug, Clone)]
pub struct ModDescription {
  pub mod_entry: Option<ModEntry>,
  fractal_link: button::State,
  nexus_link: button::State,
  file_link: button::State,
}

#[derive(Debug, Clone)]
pub enum ModDescriptionMessage {
  ModChanged(ModEntry),
  LinkClicked(String),
  FileClicked(PathBuf),
}

impl ModDescription {
  pub fn new() -> Self {
    ModDescription {
      mod_entry: None,
      fractal_link: button::State::new(),
      nexus_link: button::State::new(),
      file_link: button::State::new(),
    }
  }

  pub fn update(&mut self, message: ModDescriptionMessage) -> Command<ModDescriptionMessage> {
    match message {
      ModDescriptionMessage::ModChanged(entry) => {
        self.mod_entry = Some(entry)
      },
      ModDescriptionMessage::LinkClicked(url) => {
        if let Err(_) = opener::open(url) {
          util::error(format!("Failed to open update link. This could be due to a number of issues unfortunately.\nMake sure you have a default browser set for your operating system, otherwise there's not much that can be done."))
        }
      },
      ModDescriptionMessage::FileClicked(path) => {
        if let Err(_) = opener::open(path) {
          util::error(format!("Failed to open mod path."))
        }
      }
    }

    Command::none()
  }

  pub fn view(&mut self) -> Element<ModDescriptionMessage> {
    let mut text: Vec<Element<ModDescriptionMessage>> = vec![];

    if let Some(entry) = &self.mod_entry {
      text.push(Row::new()
        .push(Text::new(format!("Name:")).width(Length::FillPortion(1)))
        .push(Text::new(format!("{}", entry.name)).width(Length::FillPortion(10)))
        .into()
      );
      text.push(Row::new()
        .push(Text::new(format!("ID:")).width(Length::FillPortion(1)))
        .push(Text::new(format!("{}", entry.id)).width(Length::FillPortion(10)))
        .into()
      );
      text.push(Row::new()
        .push(Text::new(format!("Author(s):")).width(Length::FillPortion(1)))
        .push(Text::new(format!("{}", entry.author)).width(Length::FillPortion(10)))
        .into()
      );
      text.push(Row::new()
        .push(Text::new(format!("Enabled:")).width(Length::FillPortion(1)))
        .push(Text::new(format!("{}", if entry.enabled {
          "TRUE"
        } else {
          "FALSE"
        })).width(Length::FillPortion(10)))
        .into()
      );
      text.push(Row::new()
        .push(Text::new(format!("Version:")).width(Length::FillPortion(1)))
        .push(Text::new(format!("{}", entry.version)).width(Length::FillPortion(10)))
        .into()
      );

      if let (Some(version), _) | (None, Some(version)) = (&entry.remote_version, &entry.version_checker) {
        dbg!(version);
        if version.fractal_id.len() > 0 {
          text.push(Row::new()
            .push(Text::new(format!("Forum post:")).width(Length::FillPortion(1)))
            .push(
              Row::new()
                .push(
                  Button::new(
                    &mut self.fractal_link,
                    Text::new(format!("{}{}", ModDescription::FRACTAL_URL, version.fractal_id))
                  )
                  .padding(0)
                  .style(style::hyperlink_block::Button)
                  .width(Length::Shrink)
                  .on_press(ModDescriptionMessage::LinkClicked(format!("{}{}", ModDescription::FRACTAL_URL, version.fractal_id)))
                )
                .push(Space::with_width(Length::Fill))
                .width(Length::FillPortion(10))
            )
            .into()
          );
        }
        if version.nexus_id.len() > 0 {
          text.push(Row::new()
            .push(Text::new(format!("Nexus post:")).width(Length::FillPortion(1)))
            .push(
              Row::new()
                .push(
                  Button::new(
                    &mut self.nexus_link,
                    Text::new(format!("{}{}", ModDescription::NEXUS_URL, version.nexus_id))
                  )
                  .padding(0)
                  .style(style::hyperlink_block::Button)
                  .width(Length::Shrink)
                  .on_press(ModDescriptionMessage::LinkClicked(format!("{}{}", ModDescription::NEXUS_URL, version.nexus_id)))
                )
                .push(Space::with_width(Length::Fill))
                .width(Length::FillPortion(10))
            )
            .into()
          );
        }
      }

      text.push(Text::new(format!("Description:")).into());
      text.push(Text::new(entry.description.clone()).into());

      text.push(Row::new()
        .push(Space::with_width(Length::Fill))
        .push(
          Button::new(
            &mut self.file_link,
            Text::new(format!("Open in system file manager..."))
          )
          .width(Length::Shrink)
          .on_press(ModDescriptionMessage::FileClicked(entry.path.clone()))
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_items(Align::End)
        .into()
      );
    } else {
      text.push(Text::new(format!("No mod selected.")).into());
    }

    Column::with_children(text)
      .padding(5)
      .into()
  }

  const FRACTAL_URL: &'static str = "https://fractalsoftworks.com/forum/index.php?topic=";
  const NEXUS_URL: &'static str = "https://www.nexusmods.com/starsector/mods/";
}

#[derive(Serialize, Deserialize)]
pub struct EnabledMods {
  #[serde(rename = "enabledMods")]
  enabled_mods: Vec<String>
}

impl EnabledMods {
  pub async fn save(self, path: PathBuf) -> Result<(), SaveError> {
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    let json = serde_json::to_string_pretty(&self)
      .map_err(|_| SaveError::FormatError)?;

    let mut file = fs::File::create(path)
      .await
      .map_err(|_| SaveError::FileError)?;

    file.write_all(json.as_bytes())
      .await
      .map_err(|_| SaveError::WriteError)
  }
}

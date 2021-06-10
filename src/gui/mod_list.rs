use std::{
  io,
  io::{Read, BufReader, BufRead},
  path::PathBuf, collections::BTreeMap,
  fs::{read_dir, remove_dir_all, create_dir_all, copy, File},
  fmt::Display
};
use iced::{
  Text, Column, Command, Element, Length, Row, Scrollable, scrollable, Button,
  button, Checkbox, Container, Rule, PickList, pick_list, Space, Tooltip,
  tooltip
};
use serde::{Serialize, Deserialize};
use json_comments::strip_comments;
use json5;
use handwritten_json;
use if_chain::if_chain;
use native_dialog::{FileDialog, MessageDialog, MessageType};

use serde_aux::prelude::*;

use crate::gui::install;
use crate::style;
use crate::gui::SaveError;

pub struct ModList {
  root_dir: Option<PathBuf>,
  pub mods: BTreeMap<String, ModEntry>,
  scroll: scrollable::State,
  mod_description: ModDescription,
  install_state: pick_list::State<InstallOptions>,
  currently_highlighted: Option<String>
}

#[derive(Debug, Clone)]
pub enum ModListMessage {
  SetRoot(Option<PathBuf>),
  ModEntryMessage(String, ModEntryMessage),
  ModDescriptionMessage(ModDescriptionMessage),
  InstallPressed(InstallOptions),
  EnabledModsSaved(Result<(), SaveError>),
  ModInstalled(Result<String, install::InstallError>),
  MasterVersionReceived((String, Result<Option<ModVersion>, String>))
}

impl ModList {
  pub fn new() -> Self {
    ModList {
      root_dir: None,
      mods: BTreeMap::new(),
      scroll: scrollable::State::new(),
      mod_description: ModDescription::new(),
      install_state: pick_list::State::default(),
      currently_highlighted: None
    }
  }

  pub fn update(&mut self, message: ModListMessage) -> Command<ModListMessage> {
    match message {
      ModListMessage::SetRoot(root_dir) => {
        self.root_dir = root_dir;

        Command::batch(self.parse_mod_folder())
      },
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
          let diag = FileDialog::new().set_location(&root_dir);

          match opt {
            InstallOptions::FromArchive => {
              let mut filters = vec!["zip", "rar"];
              if cfg!(unix) {
                filters.push("7z");
              }
              if let Ok(paths) = diag.add_filter("Archive types", &filters).show_open_multiple_file() {
                return Command::batch(paths.iter().map(|path| {
                  Command::perform(install::handle_archive(path.to_path_buf(), root_dir.clone(), false, false), ModListMessage::ModInstalled)
                }))
              }

              Command::none()
            },
            InstallOptions::FromFolder => {
              match diag.show_open_single_dir() {
                Ok(Some(source_path)) => {
                  return Command::perform(install::handle_archive(source_path.to_path_buf(), root_dir.clone(), true, false), ModListMessage::ModInstalled)
                },
                Ok(None) => {},
                _ => { ModList::make_alert("Experienced an error. Did not move given folder into mods directory.".to_owned()); }
              }

              Command::none()
            },
            _ => Command::none()
          }
        } else {
          ModList::make_alert("No install directory set. Please set the Starsector install directory in Settings.".to_string());
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
      ModListMessage::ModInstalled(res) => {
        let is_err = res.is_err();
        match res {
          Ok(mod_name) | Err(install::InstallError::DeleteError(mod_name)) => {
            ModList::make_alert(format!("Successfully installed {}{}", mod_name, if is_err {".\nFailed to clean up temporary directory"} else {""}));

            Command::batch(self.parse_mod_folder())
          },
          Err(err) => {
            match err {
              install::InstallError::DirectoryExists(path, is_folder) => {
                if_chain! {
                  if let Some(_file_name) = path.file_stem();
                  if let Some(root_dir) = self.root_dir.clone();
                  let mod_dir = root_dir.join("mods");
                  let raw_dest = mod_dir.join(_file_name);
                  then {
                    match ModList::make_query(format!("A directory named {:?} already exists. Do you want to replace it?\nChoosing no will abort this operation.", _file_name)) {
                      Ok(true) => {
                        if remove_dir_all(&raw_dest).is_ok() {
                          Command::perform(install::handle_archive(path.to_path_buf(), root_dir.clone(), is_folder, false), ModListMessage::ModInstalled)
                        } else {
                          ModList::make_alert(format!("Failed to delete existing directory. Please check permissions on mod folder/{:?}", raw_dest));
                          Command::none()
                        }
                      },
                      _ => Command::none()
                    }
                  } else {
                    ModList::make_alert(format!("Encountered an error. Could not install to {:?}", path));
                    Command::none()
                  }
                }
              },
              other => {
                ModList::make_alert(format!("Encountered error: {:?}", other));
                Command::none()
              }
            }
          }
        }
      },
      ModListMessage::MasterVersionReceived((id, res)) => {
        if let Some(entry) = self.mods.get_mut(&id) {
          match res {
            Ok(maybe_version) => {
                match maybe_version {
                  Some(version) => {
                    // debug_print!("{}. ", entry.id);
                    if version.major > 0 {
                      // debug_println!("New major version available.");
                      entry.update_status = Some(UpdateStatus::Major(version))
                    } else if version.minor > 0 {
                      // debug_println!("New minor version available.");
                      entry.update_status = Some(UpdateStatus::Minor(version))
                    } else {
                      // debug_println!("New patch available.");
                      entry.update_status = Some(UpdateStatus::Patch(version.patch))
                    };
                    // debug_println!("{:?}", entry.version_checker.as_ref().unwrap().version);
                  },
                  None => {
                    // debug_println!("No update available for {}.", entry.id);
                    entry.update_status = Some(UpdateStatus::UpToDate)
                  }
                }
            },
            Err(err) => {
              // debug_println!("Could not get remote update data for {}.\nError: {}", id, err);
              entry.update_status = Some(UpdateStatus::Error)
            }
          }
        };

        Command::none()
      }
    }
  }

  pub fn view(&mut self) -> Element<ModListMessage> {
    let mut every_other = true;
    let content = Column::new()
      .push::<Element<ModListMessage>>(PickList::new(
          &mut self.install_state,
          &InstallOptions::SHOW[..],
          Some(InstallOptions::Default),
          ModListMessage::InstallPressed
        ).into()
      )
      .push(Space::with_height(Length::Units(10)))
      .push(Column::new()
        .push(Row::new()
          .push(Space::with_width(Length::Units(5)))
          .push(Text::new("Enabled").width(Length::FillPortion(3)))
          .push(Space::with_width(Length::Units(5)))
          .push(Text::new("Name").width(Length::FillPortion(8)))
          .push(Space::with_width(Length::Units(5)))
          .push(Text::new("ID").width(Length::FillPortion(8)))
          .push(Space::with_width(Length::Units(5)))
          .push(Text::new("Author").width(Length::FillPortion(8)))
          .push(Space::with_width(Length::Units(5)))
          .push(Text::new("Mod Version").width(Length::FillPortion(8)))
          .push(Space::with_width(Length::Units(5)))
          .push(Text::new("Starsector Version").width(Length::FillPortion(8)))
          .height(Length::Shrink)
          .push(Space::with_width(Length::Units(10)))
        )
      )
      .push(Rule::horizontal(2).style(style::max_rule::Rule))
      .push(Scrollable::new(&mut self.scroll)
        .height(Length::FillPortion(2))
        .push(Row::new()
          .push::<Element<ModListMessage>>(if self.mods.len() > 0 {
            self.mods
              .iter_mut()
              .fold(Column::new(), |col, (id, entry)| {
                every_other = !every_other;
                let id_clone = id.clone();
                col.push(
                  entry.view(every_other).map(move |message| {
                    ModListMessage::ModEntryMessage(id_clone.clone(), message)
                  })
                )
              })
              .width(Length::Fill)
              .into()
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
      .push(Space::with_height(Length::Units(10)))
      .push(
        Container::new(self.mod_description.view().map(|message| {
          ModListMessage::ModDescriptionMessage(message)
        }))
        .height(Length::FillPortion(1))
        .width(Length::Fill)
      );

    Column::new()
      .push(content)
      .padding(5)
      .height(Length::Fill)
      .into()
  }

  #[must_use]
  fn parse_mod_folder(&mut self) -> Vec<Command<ModListMessage>>{
    self.mods.clear();

    if_chain! {
      if let Some(root_dir) = &self.root_dir;
      let mod_dir = root_dir.join("mods");
      let enabled_mods_filename = mod_dir.join("enabled_mods.json");
      // Note: If the enabled_mods.json file does not exist or is malformed, this entire function call fails.
      if let Ok(enabled_mods_text) = std::fs::read_to_string(enabled_mods_filename);
      if let Ok(EnabledMods { enabled_mods }) = serde_json::from_str::<EnabledMods>(&enabled_mods_text);
      // Whilst that shouldn't happen (Starsector should make the file) manual deletion, manual instantiation of the mods folder, or some other error, can cause this to go poorly - consider generating ourselves.
      if let Ok(dir_iter) = std::fs::read_dir(mod_dir);
      then {
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

            let mod_info_path = entry.path().join("mod_info.json");
            if_chain! {
              if let Ok(mod_info_file) = std::fs::read_to_string(mod_info_path.clone());
              let mut stripped = String::new();
              if strip_comments(mod_info_file.as_bytes()).read_to_string(&mut stripped).is_ok();
              if let Ok(mut mod_info) = json5::from_str::<ModEntry>(&stripped);
              then {
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
                None
              }
            }
          })
          .unzip();

        self.mods.extend(mods);

        versions.iter()
          .filter_map(|v| v.as_ref())
          .map(|v| Command::perform(install::get_master_version(v.clone()), ModListMessage::MasterVersionReceived))
          .collect()
      } else {
        // debug_println!("Fatal. Could not parse mods folder. Alert developer");
        vec![]
      }
    }
  }

  pub fn make_alert(message: String) -> Result<(), String> {
    let mbox = move || {
      MessageDialog::new()
      .set_title("Alert:")
      .set_type(MessageType::Info)
      .set_text(&message)
      .show_alert()
      .map_err(|err| { err.to_string() })
    };

    // On windows we need to spawn a thread as the msg doesn't work otherwise
    #[cfg(target_os = "windows")]
    let res = match std::thread::spawn(move || {
      mbox()
    }).join() {
      Ok(Ok(())) => Ok(()),
      Ok(Err(err)) => Err(err),
      Err(err) => Err(err).map_err(|err| format!("{:?}", err))
    };

    #[cfg(not(target_os = "windows"))]
    let res = mbox();

    res
  }

  pub fn make_query(message: String) -> Result<bool, String> {
    let mbox = move || {
      MessageDialog::new()
      .set_type(MessageType::Warning)
      .set_text(&message)
      .show_confirm()
      .map_err(|err| { err.to_string() })
    };

    // On windows we need to spawn a thread as the msg doesn't work otherwise
    #[cfg(target_os = "windows")]
    let res = match std::thread::spawn(move || {
      mbox()
    }).join() {
      Ok(Ok(confirm)) => Ok(confirm),
      Ok(Err(err)) => Err(err),
      Err(err) => Err(err).map_err(|err| format!("{:?}", err))
    };

    #[cfg(not(target_os = "windows"))]
    let res = mbox();

    res
  }

  fn find_nested_mod(dest: &PathBuf) -> Result<Option<PathBuf>, io::Error> {
    for entry in read_dir(dest)? {
      let entry = entry?;
      if entry.file_type()?.is_dir() {
        let res = ModList::find_nested_mod(&entry.path())?;
        if res.is_some() { return Ok(res) }
      } else if entry.file_type()?.is_file() {
        if entry.file_name() == "mod_info.json" {
          return Ok(Some(dest.to_path_buf()));
        }
      }
    }

    Ok(None)
  }

  fn copy_dir_recursive(to: &PathBuf, from: &PathBuf) -> io::Result<()> {
    if !to.exists() {
      create_dir_all(to)?;
    }

    for entry in from.read_dir()? {
      let entry = entry?;
      if entry.file_type()?.is_dir() {
        ModList::copy_dir_recursive(&to.to_path_buf().join(entry.file_name()), &entry.path())?;
      } else if entry.file_type()?.is_file() {
        copy(entry.path(), &to.to_path_buf().join(entry.file_name()))?;
      }
    };

    Ok(())
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InstallOptions {
  FromArchive,
  FromFolder,
  Default
}

impl InstallOptions {
  const SHOW: [InstallOptions; 2] = [
    InstallOptions::FromArchive,
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
        InstallOptions::FromArchive => "From Archive",
        InstallOptions::FromFolder => "From Folder"
      }
    )
  }
}

#[derive(Debug, Clone)]
pub enum UpdateStatus {
  Major(ModVersion),
  Minor(ModVersion),
  Patch(String),
  UpToDate,
  Error
}

impl Display for UpdateStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    match self {
      UpdateStatus::Major(_) => write!(f, "Major"),
      UpdateStatus::Minor(_) => write!(f, "Minor"),
      UpdateStatus::Patch(_) => write!(f, "Patch"),
      UpdateStatus::UpToDate => write!(f, "Up to date"),
      UpdateStatus::Error => write!(f, "Error"),
    }
  }
}

pub struct UpdateStatusTTPatch(pub UpdateStatus);

#[derive(Debug, Clone, Deserialize)]
pub struct ModEntry {
  pub id: String,
  name: String,
  #[serde(default)]
  author: String,
  version: String,
  description: String,
  #[serde(alias = "gameVersion")]
  game_version: String,
  #[serde(skip)]
  enabled: bool,
  #[serde(skip)]
  highlighted: bool,
  #[serde(skip)]
  version_checker: Option<ModVersionMeta>,
  #[serde(skip)]
  update_status: Option<UpdateStatus>,
  #[serde(skip)]
  #[serde(default = "button::State::new")]
  button_state: button::State
}

#[derive(Debug, Clone)]
pub enum ModEntryMessage {
  ToggleEnabled(bool),
  EntryHighlighted,
  EntryCleared
}

impl ModEntry {
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
      }
    }
  }

  pub fn view(&mut self, other: bool) -> Element<ModEntryMessage> {
    let row = Container::new(Row::new()
      .push(
        Container::new(
          Checkbox::new(self.enabled, "", move |toggled| {
            ModEntryMessage::ToggleEnabled(toggled)
          })
        )
        .center_x()
        .center_y()
        .width(Length::FillPortion(3))
        .height(Length::Fill)
      )
      .push(
        Button::new(
          &mut self.button_state,
          Row::new()
            .push(Rule::vertical(0).style(style::max_rule::Rule))
            .push(Space::with_width(Length::Units(5)))
            .push(Text::new(self.name.clone()).width(Length::Fill))
            .push(Rule::vertical(0).style(style::max_rule::Rule))
            .push(Space::with_width(Length::Units(5)))
            .push(Text::new(self.id.clone()).width(Length::Fill))
            .push(Rule::vertical(0).style(style::max_rule::Rule))
            .push(Space::with_width(Length::Units(5)))
            .push(Text::new(self.author.clone()).width(Length::Fill))
            .push(Rule::vertical(0).style(style::max_rule::Rule))
            .push::<Element<ModEntryMessage>>(
              if let Some(status) = &self.update_status {
                Container::new(
                  Tooltip::new(
                    Container::new(Row::with_children(vec![
                      Space::with_width(Length::Units(5)).into(),
                      Text::new(self.version.clone()).into()
                    ]))
                    .style(status.clone())
                    .width(Length::Fill)
                    .height(Length::Fill),
                    match status {
                      UpdateStatus::Major(delta) | UpdateStatus::Minor(delta) => {
                        let local = &self.version_checker.as_ref().unwrap().version;
                        let remote = ModVersion {
                          major: local.major + delta.major,
                          minor: local.minor + delta.minor,
                          patch: delta.patch.clone()
                        };
                        format!("{:?} update available.\nLocal: {} - Update: {}", status, local, remote)
                      },
                      UpdateStatus::Patch(delta) => format!("Patch available.\nLocal: {} - Update: {}", &self.version_checker.as_ref().unwrap().version.patch, delta),
                      UpdateStatus::UpToDate => format!("Up to date!"),
                      UpdateStatus::Error => format!("Could not retrieve remote update data.")
                    },
                    tooltip::Position::FollowCursor
                  ).style(UpdateStatusTTPatch(status.clone()))
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(1)
                .into()
              } else {
                Text::new(self.version.clone())
                  .width(Length::Fill)
                  .into()
              }
            )
            .push(Rule::vertical(0).style(style::max_rule::Rule))
            .push(Space::with_width(Length::Units(5)))
            .push(Text::new(self.game_version.clone()).width(Length::Fill))
            .height(Length::Fill)
        )
        .padding(0)
        .height(Length::Fill)
        .style(style::button_none::Button)
        .on_press(ModEntryMessage::EntryHighlighted)
        .width(Length::FillPortion(40))
      )
      .height(Length::Units(50))
    );

    if self.highlighted {
      row.style(style::highlight_background::Container)
    } else if other {
      row.style(style::alternate_background::Container)
    } else {
      row
    }.into()
  }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModVersionMeta {
  #[serde(alias="masterVersionFile")]
  pub remote_url: String,
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
  pub version: ModVersion
}

#[derive(Debug, Clone, Deserialize, PartialEq, PartialOrd)]
pub struct ModVersion {
  #[serde(deserialize_with="deserialize_number_from_string")]
  pub major: i32,
  #[serde(deserialize_with="deserialize_number_from_string")]
  pub minor: i32,
  #[serde(default)]
  #[serde(deserialize_with="deserialize_string_from_number")]
  pub patch: String
}

impl Display for ModVersion {
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
  mod_entry: Option<ModEntry>
}

#[derive(Debug, Clone)]
pub enum ModDescriptionMessage {
  ModChanged(ModEntry)
}

impl ModDescription {
  pub fn new() -> Self {
    ModDescription {
      mod_entry: None
    }
  }

  pub fn update(&mut self, message: ModDescriptionMessage) -> Command<ModDescriptionMessage> {
    match message {
      ModDescriptionMessage::ModChanged(entry) => {
        self.mod_entry = Some(entry)
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
      text.push(Text::new(format!("Description:")).into());
      text.push(Text::new(entry.description.clone()).into());
    } else {
      text.push(Text::new(format!("No mod selected.")).into());
    }

    Column::with_children(text)
      .padding(5)
      .into()
  }
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

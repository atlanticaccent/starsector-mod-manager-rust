use std::{io, io::Read, path::PathBuf, collections::HashMap, fs::{read_dir, rename, remove_dir_all}};
use iced::{Text, Column, Command, Element, Length, Row, Scrollable, scrollable, Button, button, Checkbox, Container};
use json_comments::strip_comments;
use json5;
use if_chain::if_chain;
use native_dialog::{FileDialog, MessageDialog, MessageType};

use super::InstallOptions;
use crate::archive_handler;
use crate::style;

pub struct ModList {
  root_dir: Option<PathBuf>,
  mods: HashMap<String, ModEntry>,
  scroll: scrollable::State,
  mod_description: ModDescription
}

#[derive(Debug, Clone)]
pub enum ModListMessage {
  SetRoot(Option<PathBuf>),
  ModEntryMessage(String, ModEntryMessage),
  ModDescriptionMessage(ModDescriptionMessage),
  InstallPressed(InstallOptions)
}

impl ModList {
  pub fn new() -> Self {
    ModList {
      root_dir: None,
      mods: HashMap::new(),
      scroll: scrollable::State::new(),
      mod_description: ModDescription::new()
    }
  }

  pub fn update(&mut self, message: ModListMessage) -> Command<ModListMessage> {
    match message {
      ModListMessage::SetRoot(root_dir) => {
        self.root_dir = root_dir;

        self.parse_mod_folder();

        return Command::none();
      },
      ModListMessage::ModEntryMessage(id, message) => {
        if let Some(entry) = self.mods.get_mut(&id) {
          match message {
            ModEntryMessage::EntryHighlighted => {
              self.mod_description.update(ModDescriptionMessage::ModChanged(entry.clone()));
            },
            _ => {
              entry.update(message);
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
                let res: Vec<&str> = paths.iter()
                  .filter_map(|maybe_path| {
                    if_chain! {
                      if let Some(path) = maybe_path.to_str();
                      if let Some(_full_name) = maybe_path.file_name();
                      if let Some(full_name) = _full_name.to_str();
                      if let Some(_file_name) = maybe_path.file_stem();
                      let mod_dir = root_dir.join("mods");
                      let raw_dest = mod_dir.join(_file_name);
                      if let Some(dest) = raw_dest.to_str();
                      then {
                        if let Ok(true) = archive_handler::handle_archive(&path.to_owned(), &dest.to_owned()) {
                          match ModList::find_nested_mod(&raw_dest) {
                            Ok(Some(mod_path)) => {
                              if_chain! {
                                if let Ok(_) = rename(mod_path, mod_dir.join("temp"));
                                if let Ok(_) = remove_dir_all(&raw_dest);
                                if let Ok(_) = rename(mod_dir.join("temp"), raw_dest);
                                then {
                                  self.parse_mod_folder();
                                  None
                                } else {
                                  Some("Filesystem error.")
                                }
                              }
                            },
                            _ => Some("Could not find mod in provided archive.")
                          }
                        } else {
                          Some(full_name)
                        }
                      } else {
                        Some("Failed to parse file name.")
                      }
                    }
                  }).collect();

                match res.len() {
                  0 => {},
                  i if i < paths.len() => {
                    ModList::make_alert("Failed to decompress some files.".to_string());
                  },
                  _ => {
                    ModList::make_alert("Failed to decompress any of the given files.".to_string());
                  }
                };
              }

              Command::none()
            },
            InstallOptions::FromFolder => {
              Command::none()
            },
            _ => Command::none()
          }
        } else {
          ModList::make_alert("No install directory set. Please set the Starsector install directory in Settings.".to_string());
          return Command::none();
        }
      }
    }
  }

  pub fn view(&mut self) -> Element<ModListMessage> {
    let content = Column::new()
      .width(Length::FillPortion(4))
      .push(Scrollable::new(&mut self.scroll)
        .height(Length::FillPortion(2))
        .push::<Element<ModListMessage>>(if self.mods.len() > 0 {
          self.mods
            .iter_mut()
            .fold(Column::new().padding(5), |col, (id, entry)| {
              let id_clone = id.clone();
              col.push(
                entry.view().map(move |message| {
                  ModListMessage::ModEntryMessage(id_clone.clone(), message)
                })
              )
            })
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
      )
      .push(
        Container::new(self.mod_description.view().map(|message| {
          ModListMessage::ModDescriptionMessage(message)
        }))
        .height(Length::FillPortion(1))
        .width(Length::Fill)
        .style(style::border::Container)
      );

    Column::new()
      .push(content)
      .padding(5)
      .height(Length::Fill)
      .into()
  }

  fn parse_mod_folder(&mut self) {
    self.mods.clear();

    if_chain! {
      if let Some(root_dir) = &self.root_dir;
      let mod_dir = root_dir.join("mods");
      if let Ok(dir_iter) = std::fs::read_dir(mod_dir);
      then {
        let mods = dir_iter
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
              if let Ok(mod_info) = json5::from_str::<serde_json::Value>(&stripped);
              then {
                Some((
                  mod_info["id"].to_string(),
                  ModEntry::new(
                    mod_info["id"].to_string(),
                    mod_info["name"].to_string(),
                    mod_info["author"].to_string(),
                    mod_info["version"].to_string(),
                    mod_info["description"].to_string(),
                    mod_info["gameVersion"].to_string()
                  )
                ))
              } else {
                None
              }
            }
          });

        self.mods.extend(mods)
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
}

#[derive(Debug, Clone)]
pub struct ModEntry {
  pub id: String,
  name: String,
  author: String,
  version: String,
  description: String,
  game_version: String,
  enabled: bool,
  button_state: button::State
}

#[derive(Debug, Clone)]
pub enum ModEntryMessage {
  ToggleEnabled(bool),
  EntryHighlighted
}

impl ModEntry {
  pub fn new(id: String, name: String, author: String, version: String, description: String, game_version: String) -> Self {
    ModEntry {
      id,
      name,
      author,
      version,
      description,
      game_version,
      enabled: false,
      button_state: button::State::new()
    }
  }

  pub fn update(&mut self, message: ModEntryMessage) -> Command<ModEntryMessage> {
    match message {
      ModEntryMessage::ToggleEnabled(enabled) => {
        self.enabled = enabled;

        Command::none()
      },
      ModEntryMessage::EntryHighlighted => {
        Command::none()
      }
    }
  }

  pub fn view(&mut self) -> Element<ModEntryMessage> {
    Row::new()
      .push(Checkbox::new(self.enabled, "", move |toggled| {
        ModEntryMessage::ToggleEnabled(toggled)
      }).width(Length::Shrink))
      .push(
        Button::new(
          &mut self.button_state,
          Row::new()
            .push(Text::new(self.name.clone()).width(Length::Fill))
            .push(Text::new(self.id.clone()).width(Length::Fill))
            .push(Text::new(self.author.clone()).width(Length::Fill))
            .push(Text::new(self.version.clone()).width(Length::Fill))
            .push(Text::new(self.game_version.clone()).width(Length::Fill))
        )
        .style(style::button_none::Button)
        .on_press(ModEntryMessage::EntryHighlighted)
      )
      .padding(5)
      .into()
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
    Row::new()
      .push(Text::new(if let Some(entry) = &self.mod_entry {
        entry.description.clone()
      } else {
        "No mod selected.".to_owned()
      }))
      .padding(5)
      .into()
  }
}

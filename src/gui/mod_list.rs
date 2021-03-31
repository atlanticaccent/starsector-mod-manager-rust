use std::{io::Read, path::PathBuf, collections::HashMap};
use iced::{Text, Column, Command, Element, Length, Row, Rule, Scrollable, scrollable, Button, button, Checkbox, Align, PickList, pick_list, Container};
use json_comments::strip_comments;
use json5;
use if_chain::if_chain;
use native_dialog::{FileDialog, MessageDialog, MessageType};

use crate::archive_handler;

pub struct ModList {
  root_dir: Option<PathBuf>,
  mods: HashMap<String, ModEntry>,
  scroll: scrollable::State,
  mod_description: ModDescription,
  install_state: pick_list::State<InstallOptions>
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
      mod_description: ModDescription::new(),
      install_state: pick_list::State::default()
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
        let diag = FileDialog::new().set_location("~/Desktop");

        match opt {
          InstallOptions::FromArchive => {
            if self.root_dir.is_none() {
              ModList::make_alert("No install directory set. Please set the Starsector install directory in Settings.".to_string());
              return Command::none();
            }

            let mut filters = vec!["zip", "rar"];
            if cfg!(unix) {
              filters.push("7z");
            }
            if let Ok(paths) = diag.add_filter("Archive types", &filters).show_open_multiple_file() {
              let res: Vec<String> = paths.iter()
                .filter_map(|maybe_path| {
                  if_chain! {
                    if let Some(path) = maybe_path.to_str();
                    if let Some(_full_name) = maybe_path.file_name();
                    if let Some(full_name) = _full_name.to_str();
                    if let Some(_file_name) = maybe_path.file_stem();
                    if let Some(file_name) = _file_name.to_str();
                    if let Err(_) = archive_handler::handle_archive(&path.to_owned(), &file_name.to_string());
                    then {
                      Some(full_name.to_string())
                    } else {
                      None
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
  
    let controls: Column<ModListMessage> = Column::new()
      .width(Length::FillPortion(1))
      .padding(20)
      .align_items(Align::Center)
      .push::<Element<ModListMessage>>(
        PickList::new(
          &mut self.install_state,
          &InstallOptions::SHOW[..],
          Some(InstallOptions::Default),
          ModListMessage::InstallPressed
        ).into()
      );

    Row::new()
      .push(content)
      .push(Rule::vertical(1))
      .push(controls)
      .padding(5)
      .width(Length::Fill)
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
        .style(style::Theme::None)
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
        "".to_owned()
      }))
      .padding(5)
      .into()
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

pub mod style {
  use iced::{button};

  pub enum Theme {
    None
  }

  impl From<Theme> for Box<dyn button::StyleSheet> {
    fn from(theme: Theme) -> Self {
      match theme {
        Theme::None => none::Button.into(),
      }
    }
  }

  mod none {
    use iced::{button, Color, Vector};

    pub struct Button;

    impl button::StyleSheet for Button {
      fn active(&self) -> button::Style {
        button::Style {
          background: Color::from_rgb(255.0, 255.0, 255.0).into(),
          border_radius: 12.0,
          shadow_offset: Vector::new(0.0, 0.0),
          text_color: Color::from_rgb(0.0, 0.0, 0.0),
          ..button::Style::default()
        }
      }
    
      fn hovered(&self) -> button::Style {
        button::Style {
          ..self.active()
        }
      }
    }
  }

  pub mod border {
    use iced::{container, Color};

    pub struct Container;

    impl container::StyleSheet for Container {
      fn style(&self) -> container::Style {
        container::Style {
          border_width: 0.5,
          border_color: Color::from_rgb(0.0, 0.0, 0.0),
          ..container::Style::default()
        }
      }
    }
  }
}

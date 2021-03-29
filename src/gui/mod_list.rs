use std::{io::Read, path::PathBuf, collections::HashMap};
use iced::{Text, Column, Command, Element, Length, Row, Rule, Scrollable, scrollable};
use json_comments::strip_comments;
use json5;
use if_chain::if_chain;

pub struct ModList {
  root_dir: Option<PathBuf>,
  mods: HashMap<String, ModEntry>,
  scroll: scrollable::State
}

#[derive(Debug, Clone)]
pub enum ModListMessage {
  SetRoot(Option<PathBuf>),
  ModEntryMessage(String, ModEntryMessage)
}

impl ModList {
  pub fn new() -> Self {
    ModList {
      root_dir: None,
      mods: HashMap::new(),
      scroll: scrollable::State::new()
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
          entry.update(message);
        }

        return Command::none();
      }
    }
  }

  pub fn view(&mut self) -> Element<ModListMessage> {
    let list: Scrollable<ModListMessage> = Scrollable::new(&mut self.scroll)
      .width(Length::FillPortion(4))
      .push::<Element<ModListMessage>>(if self.mods.len() > 0 {
        self.mods
          .iter_mut()
          .fold(Column::new().padding(20), |col, (_, entry)| {
            col.push(entry.view().map(|message| {
              ModListMessage::ModEntryMessage(message.id, message.message)
            }))
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
      });
  
    let controls: Column<ModListMessage> = Column::new()
      .width(Length::FillPortion(1));

    Row::new()
      .push(list)
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
}

#[derive(Debug, Clone)]
pub struct ModEntry {
  pub id: String,
  name: String,
  author: String,
  version: String,
  description: String,
  game_version: String,
  enabled: bool
}

#[derive(Debug, Clone)]
pub struct ModEntryMessageStruct {
  pub id: String,
  pub message: ModEntryMessage
}

#[derive(Debug, Clone)]
pub enum ModEntryMessage {

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
      enabled: false
    }
  }

  pub fn update(&mut self, message: ModEntryMessage) -> Command<ModEntryMessage> {
    Command::none()
  }

  pub fn view(&mut self) -> Element<ModEntryMessageStruct> {
    Row::new()
      .push(Text::new(self.id.clone()))
      .into()
  }
}

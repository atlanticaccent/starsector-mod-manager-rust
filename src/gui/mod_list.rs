use iced::{Column, Command, Element, Length, Row, Rule};

pub struct ModList {
  mods: Vec<mod_entry::ModEntry>
}

#[derive(Debug, Clone)]
pub enum ModListMessage {

}

impl ModList {
  pub fn new() -> Self {
    ModList {
      mods: Vec::new()
    }
  }

  pub fn update(&mut self, message: ModListMessage) -> Command<ModListMessage> {
    Command::none()
  }

  pub fn view(&mut self) -> Element<ModListMessage> {
    let list: Column<ModListMessage> = Column::new()
        .width(Length::FillPortion(4));
  
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
}

mod mod_entry {
  use iced::Command;

  pub struct ModEntry {
    id: String,
    name: String,
    author: String,
    version: String,
    description: String,
    game_version: String,
  }

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
        game_version
      }
    }

    pub fn update(&mut self, message: ModEntryMessage) -> Command<ModEntryMessage> {
      Command::none()
    }

    pub fn view(&mut self) {

    }
  }
}

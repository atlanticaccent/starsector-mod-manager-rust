use iced::{Column, Command, Element, Length, Row, Rule};

pub struct ModList {

}

#[derive(Debug, Clone)]
pub enum ModListMessage {

}

impl ModList {
  pub fn new() -> Self {
    ModList {

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
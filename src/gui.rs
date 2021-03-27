use iced::{Application, button, Button, Column, Command, Element, Length, Row, Rule, Text, executor};
// use std::path::PathBuf;

use self::settings::SettingsMessage;

mod settings;

pub struct App {
  // root_dir: Option<PathBuf>,
  settings_button: button::State,
  settings_open: bool,
  settings: settings::Settings
}

#[derive(Debug, Clone)]
pub enum Message {
  SettingsOpen,
  SettingsClose,
  SettingsMessage(SettingsMessage)
}

impl Application for App {
  type Executor = executor::Default;
  type Message = Message;
  type Flags = ();
  
  fn new(_flags: ()) -> (App, Command<Message>) {
    (
      App {
        // root_dir: None,
        settings_button: button::State::new(),
        settings_open: false,
        settings: settings::Settings::new()
      },
      Command::none()
    )
  }
  
  fn title(&self) -> String {
    String::from("Starsector Mod Manager")
  }
  
  fn update(&mut self, _message: Message) -> Command<Message> {
    match _message {
      Message::SettingsOpen => {
        self.settings_open = true;
        // self.settings.update(SettingsMessage::InitRoot(self.root_dir.clone()));
        return Command::none();
      },
      Message::SettingsClose => {
        self.settings_open = false;

        self.settings.update(SettingsMessage::Close);

        return Command::none();
      }
      Message::SettingsMessage(settings_message) => {
        self.settings.update(settings_message);
        return Command::none();
      }
    }
  }
  
  fn view(&mut self) -> Element<Message> {
    let menus: Row<Message> = Row::new()
      .push(
        if self.settings_open {
          Button::new(
            &mut self.settings_button, 
            Text::new("Go back"),
          )
          .on_press(
            Message::SettingsClose
          )
        } else {
          Button::new(
            &mut self.settings_button, 
            Text::new("Settings"),
          )
          .on_press(
            Message::SettingsOpen
          )
        }
      );

    let content: Element<Message> = if self.settings_open {
      self.settings.view().map(move |_message| {
        Message::SettingsMessage(_message)
      })
    } else {
      let list: Column<Message> = Column::new()
        .width(Length::FillPortion(4));
  
      let controls: Column<Message> = Column::new()
        .width(Length::FillPortion(1));

      Row::new()
        .push(list)
        .push(Rule::vertical(1))
        .push(controls)
        .padding(5)
        .width(Length::Fill)
        .into()
    };

    Column::new()
      .push(menus)
      .push(content)
      .width(Length::Fill)
      .into()
  }
}

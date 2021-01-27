use iced::{Align, Application, button, Button, Column, Command, Element, Length, Row, Rule, Space, Text, executor, text_input, TextInput};
use std::path::PathBuf;

pub struct App {
  root_dir: Option<PathBuf>,
  path_input_state: text_input::State,
  settings_button: button::State,
  settings_open: bool,
  browse_button: button::State,
}

#[derive(Debug, Clone)]
pub enum Message {
  SettingsOpen,
  SettingsClose,
  PathChanged(String),
  OpenNativeDiag
}

impl Application for App {
  type Executor = executor::Default;
  type Message = Message;
  type Flags = ();
  
  fn new(_flags: ()) -> (App, Command<Message>) {
    (
      App {
        root_dir: None,
        path_input_state: text_input::State::new(),
        settings_button: button::State::new(),
        settings_open: false,
        browse_button: button::State::new(),
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
        return Command::none();
      },
      Message::SettingsClose => {
        self.settings_open = false;
        return Command::none();
      }
      Message::PathChanged(some_path) => {
        let path = PathBuf::from(some_path);

        if (*path).exists() {
          self.root_dir = Some(path);
        }

        return Command::none();
      },
      Message::OpenNativeDiag => {
        

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

    let list: Column<Message> = Column::new()
      .width(Length::FillPortion(4));

    let controls: Column<Message> = Column::new()
      .width(Length::FillPortion(1));

    let content: Row<Message> = if self.settings_open {
      let input = TextInput::new(
        &mut self.path_input_state,
        "/",
        if let Some(path) = &self.root_dir {
          path.to_str().unwrap_or("")
        } else {
          ""
        },
        |path| -> Message { 
          Message::PathChanged(path)
        }
      )
      .padding(5);

      let browse = Button::new(
        &mut self.browse_button,
        Text::new("Browse ...")
      )
      .on_press(Message::OpenNativeDiag);

      Row::new().push(
        Row::new()
          .push(Text::new("Starsector Install Dir: "))
          .push(input)
          .push(browse)
          .width(Length::Fill)
          .height(Length::Fill)
          .align_items(Align::Center)
      )
    } else {
      Row::new()
        .push(list)
        .push(Rule::vertical(1))
        .push(controls)
        .padding(5)
        .width(Length::Fill)
    };
    

    Column::new()
      .push(menus)
      .push(content)
      .width(Length::Fill)
      .into()
  }
}

use iced::{Align, Application, button, Button, Column, Command, Element, Length, Row, Rule, Space, Text, executor, text_input, TextInput};
use native_dialog::{FileDialog};
use std::path::PathBuf;

pub struct App {
  root_dir: Option<PathBuf>,
  new_dir: Option<String>,
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
        new_dir: None,
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

        let some_path = PathBuf::from(&self.new_dir.as_deref().unwrap_or_else(|| ""));

        if (*some_path).exists() {
          self.root_dir.replace(some_path);
        } else {
          self.new_dir = None;
        }

        return Command::none();
      }
      Message::PathChanged(path) => {
        self.new_dir.replace(path);
        return Command::none();
      },
      Message::OpenNativeDiag => {
        let diag = FileDialog::new().set_location("~/Desktop");

        if let Ok(some_path) = diag.show_open_single_dir() {
          if let Some(ref path_buf) = some_path {
            self.new_dir = Some(path_buf.to_string_lossy().into_owned())
          }
          
          self.root_dir = some_path;
        }

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

    let tmp;
    let content: Row<Message> = if self.settings_open {
      let input = TextInput::new(
        &mut self.path_input_state,
        "/",
        match self.new_dir {
          Some(ref value) => value.as_str(),
          None => {
            match self.root_dir {
              Some(ref root) => {
                tmp = root.display().to_string();

                tmp.as_str()
              },
              None => "",
            }
          }
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
      ).padding(5)
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

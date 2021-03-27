use iced::{Align, Button, Length, Text, TextInput};
use::iced::{Command, Row, Element, text_input, button};
use native_dialog::{FileDialog};
use std::path::PathBuf;

pub struct Settings {
  pub root_dir: Option<PathBuf>,
  pub new_dir: Option<String>,
  path_input_state: text_input::State,
  browse_button: button::State,
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
  InitRoot(Option<PathBuf>),
  PathChanged(String),
  OpenNativeDiag
}

impl Settings {
  pub fn new() -> Self {
    Settings {
      root_dir: None,
      new_dir: None,
      path_input_state: text_input::State::new(),
      browse_button: button::State::new()
    }
  }

  pub fn update(&mut self, message: SettingsMessage) -> Command<SettingsMessage> {
    match message {
      SettingsMessage::InitRoot(mut _root_dir) => {
        self.root_dir = _root_dir.take();
        return Command::none();
      },
      SettingsMessage::PathChanged(path) => {
        self.new_dir.replace(path);
        return Command::none();
      },
      SettingsMessage::OpenNativeDiag => {
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

  pub fn view(&mut self) -> Element<SettingsMessage> {
    let tmp;
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
      |path| -> SettingsMessage { 
        SettingsMessage::PathChanged(path)
      }
    )
    .padding(5);
  
    let browse = Button::new(
      &mut self.browse_button,
      Text::new("Browse ...")
    )
    .on_press(SettingsMessage::OpenNativeDiag);
  
    Row::new().push(
      Row::new()
        .push(Text::new("Starsector Install Dir: "))
        .push(input)
        .push(browse)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_items(Align::Center)
    )
    .padding(5)
    .into()
  }
}
use std::path::PathBuf;
use iced::{Application, button, Button, Column, Command, Element, Length, Row, Text, executor, PickList, pick_list, widget::Space, Clipboard};
use serde::{Serialize, Deserialize};
use serde_json;

mod settings;
mod mod_list;

use crate::style;

use settings::SettingsMessage;
use mod_list::ModListMessage;

pub struct App {
  config: Option<Config>,
  settings_button: button::State,
  settings_open: bool,
  install_state: pick_list::State<InstallOptions>,
  settings: settings::Settings,
  mod_list: mod_list::ModList
}

#[derive(Debug, Clone)]
pub enum Message {
  ConfigLoaded(Result<Config, LoadError>),
  ConfigSaved(Result<(), SaveError>),
  SettingsOpen,
  SettingsClose,
  SettingsMessage(SettingsMessage),
  ModListMessage(ModListMessage)
}

impl Application for App {
  type Executor = executor::Default;
  type Message = Message;
  type Flags = ();
  
  fn new(_flags: ()) -> (App, Command<Message>) {
    (
      App {
        config: None,
        settings_button: button::State::new(),
        settings_open: false,
        install_state: pick_list::State::default(),
        settings: settings::Settings::new(),
        mod_list: mod_list::ModList::new()
      },
      Command::perform(Config::load(), Message::ConfigLoaded)
    )
  }
  
  fn title(&self) -> String {
    String::from("Starsector Mod Manager")
  }
  
  fn update(
    &mut self,
    _message: Message,
    _clipboard: &mut Clipboard,
  ) -> Command<Message> {
    match _message {
      Message::ConfigLoaded(res) => {
        match res {
          Ok(config) => {
            self.settings.update(SettingsMessage::InitRoot(config.install_dir.clone()));

            self.mod_list.update(ModListMessage::SetRoot(config.install_dir.clone()));

            self.config = Some(config);
          },
          Err(err) => {
            println!("{:?}", err);

            self.settings.update(SettingsMessage::InitRoot(None));

            self.config = Some(Config {
              install_dir: None
            })
          }
        }


        Command::none()
      },
      Message::ConfigSaved(res) => {
        match res {
          Err(err) => println!("{:?}", err),
          _ => {}
        }

        Command::none()
      },
      Message::SettingsOpen => {
        self.settings_open = true;
        // self.settings.update(SettingsMessage::InitRoot(self.root_dir.clone()));
        return Command::none();
      },
      Message::SettingsClose => {
        self.settings_open = false;

        self.settings.update(SettingsMessage::Close);

        self.mod_list.update(ModListMessage::SetRoot(self.settings.root_dir.clone()));

        if let Some(config) = self.config.as_mut() {
          config.install_dir = self.settings.root_dir.clone();

          return Command::perform(config.clone().save(), Message::ConfigSaved);
        }

        Command::none()
      } 
      Message::SettingsMessage(settings_message) => {
        self.settings.update(settings_message);
        return Command::none();
      },
      Message::ModListMessage(mod_list_message) => {
        return self.mod_list.update(mod_list_message.clone()).map(|m| Message::ModListMessage(m));
      }
    }
  }
  
  fn view(&mut self) -> Element<Message> {
    let mut menus: Row<Message> = Row::new();

    menus = if self.settings_open {
      menus.push(
        Button::new(
          &mut self.settings_button, 
          Text::new("Go back"),
        )
        .on_press(
          Message::SettingsClose
        )
        .style(style::button_only_hover::Button)
      )
    } else {
      menus.push(
        Button::new(
          &mut self.settings_button, 
          Text::new("Settings"),
        )
        .on_press(
          Message::SettingsOpen
        )
        .style(style::button_only_hover::Button)
      ).push(
        Space::with_width(Length::Fill)
      ).push({
        let pick: Element<ModListMessage> = PickList::new(
          &mut self.install_state,
          &InstallOptions::SHOW[..],
          Some(InstallOptions::Default),
          ModListMessage::InstallPressed
        ).into();
        pick.map(|m| Message::ModListMessage(m))
      })
    };

    let content: Element<Message> = if self.settings_open {
      self.settings.view().map(move |_message| {
        Message::SettingsMessage(_message)
      })
    } else {
      self.mod_list.view().map(move |_message| {
        Message::ModListMessage(_message)
      })
    };

    Column::new()
      .push(menus)
      .push(content)
      .width(Length::Fill)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadError {
  NoSuchFile,
  ReadError,
  FormatError
}

#[derive(Debug, Clone)]
pub enum SaveError {
  FileError,
  WriteError,
  FormatError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
  install_dir: Option<PathBuf>
}

impl Config {
  fn path() -> PathBuf {
    PathBuf::from(r"./config.json")
  }

  async fn load() -> Result<Config, LoadError> {
    use tokio::fs;
    use tokio::io::AsyncReadExt;

    let mut config_file = fs::File::open(Config::path())
      .await
      .map_err(|_| LoadError::NoSuchFile)?;

    let mut config_string = String::new();
    config_file.read_to_string(&mut config_string)
      .await
      .map_err(|_| LoadError::ReadError)?;

    serde_json::from_str::<Config>(&config_string).map_err(|_| LoadError::FormatError)
  }

  async fn save(self) -> Result<(), SaveError> {
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    let json = serde_json::to_string_pretty(&self)
      .map_err(|_| SaveError::FormatError)?;

    let mut file = fs::File::create(Config::path())
      .await
      .map_err(|_| SaveError::FileError)?;

    file.write_all(json.as_bytes())
      .await
      .map_err(|_| SaveError::WriteError)
  }
}

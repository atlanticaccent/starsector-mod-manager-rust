use std::path::PathBuf;
use iced::{Application, button, Button, Column, Command, Element, Length, Row, Text, executor, Clipboard, Container, Space, Subscription};
use serde::{Serialize, Deserialize};
use serde_json;

const DEV_VERSION: &'static str = "IN_DEV";
const TAG: &'static str = env!("CARGO_PKG_VERSION");

// https://users.rust-lang.org/t/show-value-only-in-debug-mode/43686/5
macro_rules! dbg {
  ($($x:tt)*) => {
    {
      #[cfg(debug_assertions)]
      {
        std::dbg!($($x)*)
      }
      #[cfg(not(debug_assertions))]
      {
        ($($x)*)
      }
    }
  }
}

mod settings;
pub mod mod_list;
pub mod util;
mod installer;

use crate::style;

use settings::{SettingsMessage, vmparams::{VMParams, Value, Unit}};
use mod_list::ModListMessage;

pub struct App {
  config: Option<Config>,
  settings_button: button::State,
  apply_button: button::State,
  settings_open: bool,
  settings: settings::Settings,
  mod_list: mod_list::ModList,
  manager_update_status: Option<Result<String, String>>,
  manager_update_link_state: button::State,
  settings_changed: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
  ConfigLoaded(Result<Config, LoadError>),
  ConfigSaved(Result<(), SaveError>),
  VMParamsLoaded(Result<VMParams, LoadError>),
  VMParamsSaved(Result<(), SaveError>),
  SettingsOpen,
  SettingsApply(bool),
  SettingsMessage(SettingsMessage),
  ModListMessage(ModListMessage),
  TagsReceived(Result<String, String>),
  OpenReleases,
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
        apply_button: button::State::new(),
        settings_open: false,
        settings: settings::Settings::new(),
        mod_list: mod_list::ModList::new(),
        manager_update_status: None,
        manager_update_link_state: button::State::new(),
        settings_changed: false,
      },
      Command::batch(vec![
        Command::perform(Config::load(), Message::ConfigLoaded),
        Command::perform(App::get_latest_manager(), Message::TagsReceived)
      ])
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
        let mut commands = vec![];
        match res {
          Ok(config) => {
            commands.push(self.settings.update(SettingsMessage::InitRoot(config.install_dir.clone())).map(|m| Message::SettingsMessage(m)));

            commands.push(self.mod_list.update(ModListMessage::SetRoot(config.install_dir.clone())).map(|m| Message::ModListMessage(m)));
            commands.push(self.mod_list.update(ModListMessage::SetLastBrowsed(config.last_browsed.clone())).map(|m| Message::ModListMessage(m)));

            if let Some(install_dir) = &config.install_dir {
              commands.push(Command::perform(VMParams::load(install_dir.clone()), Message::VMParamsLoaded));
            }

            self.config = Some(config);
          },
          Err(err) => {
            dbg!("{:?}", err);

            commands.push(self.settings.update(SettingsMessage::InitRoot(None)).map(|m| Message::SettingsMessage(m)));

            self.config = Some(Config {
              install_dir: None,
              last_browsed: None
            })
          }
        }

        Command::batch(commands)
      },
      Message::ConfigSaved(res) => {
        match res {
          Err(err) => { dbg!("{:?}", err); },
          _ => {}
        };

        Command::none()
      },
      Message::VMParamsLoaded(res) => {
        match res {
          Ok(vmparams) => {
            self.settings.update(SettingsMessage::InitVMParams(Some(vmparams)));
          },
          Err(err) => {
            dbg!("Failed to parse vmparams.\n{:?}", err);
          }
        }
        Command::none()
      },
      Message::VMParamsSaved(res) => {
        match res {
          Err(err) => { dbg!("{:?}", err); },
          _ => {}
        };

        Command::none()
      },
      Message::SettingsOpen => {
        self.settings_open = true;

        return Command::none();
      },
      Message::SettingsApply(keep_open) => {
        self.settings_open = keep_open;
        self.settings_changed = false;

        let mut commands = vec![
          self.settings.update(SettingsMessage::Close).map(|m| Message::SettingsMessage(m)),
          self.mod_list.update(ModListMessage::SetRoot(self.settings.root_dir.clone())).map(|m| Message::ModListMessage(m))
        ];

        if let Some(config) = self.config.as_mut() {
          config.install_dir = self.settings.root_dir.clone();

          commands.push(Command::perform(config.clone().save(), Message::ConfigSaved));

          if let Some(install_dir) = &config.install_dir {
            if let Some(vmparams) = self.settings.vmparams.as_mut() {
              if vmparams.heap_init.amount == 0 {
                vmparams.heap_init = Value {
                  amount: 1536,
                  unit: Unit::Mega
                };
              };
              if vmparams.heap_max.amount == 0 {
                vmparams.heap_max = Value {
                  amount: 1536,
                  unit: Unit::Mega
                };
              };
              if vmparams.thread_stack_size.amount == 0 {
                vmparams.thread_stack_size = Value {
                  amount: 1536,
                  unit: Unit::Kilo
                };
              }

              commands.push(Command::perform(vmparams.clone().save(install_dir.clone()), Message::VMParamsSaved))
            } else {
              commands.push(Command::perform(VMParams::load(install_dir.clone()), Message::VMParamsLoaded))
            }
          }
        }

        Command::batch(commands)
      } 
      Message::SettingsMessage(settings_message) => {
        if let SettingsMessage::OpenNativeFilePick | SettingsMessage::PathChanged(_) | SettingsMessage::VMParamChanged(_, _) | SettingsMessage::UnitChanged(_, _) = settings_message {
          self.settings_changed = true;
        }

        self.settings.update(settings_message);
        return Command::none();
      },
      Message::ModListMessage(mod_list_message) => {
        let mut commands = vec![self.mod_list.update(mod_list_message.clone()).map(|m| Message::ModListMessage(m))];

        if let Some(config) = self.config.as_mut() {
          config.last_browsed = self.mod_list.last_browsed.clone();

          commands.push(Command::perform(config.clone().save(), Message::ConfigSaved));
        }

        Command::batch(commands)
      },
      Message::TagsReceived(res) => {
        self.manager_update_status = Some(res);

        Command::none()
      },
      Message::OpenReleases => {
        if let Err(_) = opener::open("https://github.com/atlanticaccent/starsector-mod-manager-rust/releases") {
          println!("Failed to open GitHub");
        }
        
        Command::none()
      }
    }
  }
  
  fn view(&mut self) -> Element<Message> {
    let mut buttons: Row<Message> = Row::new()
      .push(Space::with_width(Length::Units(5)));

    let tag = format!("v{}", TAG);
    let err_string = format!("{} Err", &tag);
    let update = match &self.manager_update_status {
      Some(Ok(_)) if &tag == DEV_VERSION => Container::new(Text::new("If you see this I forgot to set the version")).padding(5),
      Some(Ok(remote)) if remote > &tag => {
        Container::new(
          Button::new(
            &mut self.manager_update_link_state,
            Text::new("Update Available!")
          )
          .on_press(Message::OpenReleases)
          .style(style::button_only_hover::Button)
          .padding(5)
        )
      },
      Some(Ok(remote)) if remote < &tag => Container::new(Text::new("Are you from the future?")).padding(5),
      Some(Ok(_)) | None => Container::new(Text::new(&tag)).padding(5),
      Some(Err(_)) => Container::new(Text::new(&err_string)).padding(5),
    }.width(Length::Fill).align_x(iced::Align::Center);
    buttons = if self.settings_open {
      buttons
        .push(Space::with_width(Length::FillPortion(1)))
        .push(update)
        .push(Row::new()
          .push(Space::with_width(Length::Fill))
          .push({
            let button = Button::new(
              &mut self.apply_button, 
              Text::new("Apply"),
            )
            .on_press(
              Message::SettingsApply(true)
            )
            .style(style::button_highlight_and_hover::Button)
            .padding(5);

            if self.settings_changed {
              button.style(style::button_highlight_and_hover::Button)
            } else {
              button.style(style::button_only_hover::Button)
            }
          })
          .push(
            Button::new(
              &mut self.settings_button, 
              Text::new("Close"),
            )
            .on_press(
              Message::SettingsApply(false)
            )
            .style(style::button_only_hover::Button)
            .padding(5)
          )
          .width(Length::FillPortion(1))
        )
    } else {
      buttons
        .push(Row::new()
          .push(
            Button::new(&mut self.settings_button, Text::new("Settings"))
              .on_press(Message::SettingsOpen)
              .style(style::button_only_hover::Button)
              .padding(5)
          )
          .push(Space::with_width(Length::Fill))
          .width(Length::FillPortion(1))
        )
        .push(update)
        .push(Space::with_width(Length::FillPortion(1)))
    };

    let menu = Container::new(buttons)
      .style(style::nav_bar::Container)
      .width(Length::Fill);

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
      .push(menu)
      .push(content)
      .width(Length::Fill)
      .into()
  }

  fn subscription(&self) -> Subscription<Message> {
    self.mod_list.subscription().map(Message::ModListMessage)
  }
}

impl App {
  async fn get_latest_manager() -> Result<String, String> {
    #[derive(Deserialize)]
    struct Release {
      name: String
    }

    let client = reqwest::Client::builder()
      .user_agent("StarsectorModManager")
      .build()
      .map_err(|e| e.to_string())?;

    let res = client.get("https://api.github.com/repos/atlanticaccent/starsector-mod-manager-rust/tags")
      .send()
      .await
      .map_err(|e| e.to_string())?
      .json::<Vec<Release>>()
      .await
      .map_err(|e| e.to_string())?;

    if let Some(release) = res.first() {
      Ok(release.name.clone())
    } else {
      Err(format!("Could not find any releases."))
    }
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
  install_dir: Option<PathBuf>,
  last_browsed: Option<PathBuf>
}

impl Config {
  async fn path(try_make: bool) -> PathBuf {
    use directories::ProjectDirs;
    use tokio::fs;

    if let Some(proj_dirs) = ProjectDirs::from("org", "laird", "Starsector Mod Manager") {
      if proj_dirs.config_dir().exists() || (try_make && fs::create_dir_all(proj_dirs.config_dir()).await.is_ok()) {
        return proj_dirs.config_dir().to_path_buf().join("config.json");
      }
    };
    PathBuf::from(r"./config.json")
  }

  async fn load() -> Result<Config, LoadError> {
    use tokio::fs;
    use tokio::io::AsyncReadExt;

    let mut config_file = fs::File::open(Config::path(false).await)
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

    let mut file = fs::File::create(Config::path(true).await)
      .await
      .map_err(|_| SaveError::FileError)?;

    file.write_all(json.as_bytes())
      .await
      .map_err(|_| SaveError::WriteError)
  }
}

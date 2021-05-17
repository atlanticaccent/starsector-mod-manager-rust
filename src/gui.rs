use iced::{Application, button, Button, Column, Command, Element, Length, Row, Text, executor, PickList, pick_list, widget::Space, Clipboard};

mod settings;
mod mod_list;

use crate::style;

use settings::SettingsMessage;
use mod_list::ModListMessage;

pub struct App {
  // root_dir: Option<PathBuf>,
  settings_button: button::State,
  settings_open: bool,
  install_state: pick_list::State<InstallOptions>,
  settings: settings::Settings,
  mod_list: mod_list::ModList
}

#[derive(Debug, Clone)]
pub enum Message {
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
        // root_dir: None,
        settings_button: button::State::new(),
        settings_open: false,
        install_state: pick_list::State::default(),
        settings: settings::Settings::new(),
        mod_list: mod_list::ModList::new()
      },
      Command::none()
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
      Message::SettingsOpen => {
        self.settings_open = true;
        // self.settings.update(SettingsMessage::InitRoot(self.root_dir.clone()));
        return Command::none();
      },
      Message::SettingsClose => {
        self.settings_open = false;

        self.settings.update(SettingsMessage::Close);

        self.mod_list.update(ModListMessage::SetRoot(self.settings.root_dir.clone()));

        return Command::none();
      }
      Message::SettingsMessage(settings_message) => {
        self.settings.update(settings_message);
        return Command::none();
      },
      Message::ModListMessage(mod_list_message) => {
        self.mod_list.update(mod_list_message);
        
        return Command::none();
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

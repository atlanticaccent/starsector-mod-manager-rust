use iced::{Align, Button, Length, Text, TextInput, Command, Row, Column, Element, text_input, button, Container, Space};
use native_dialog::{FileDialog};
use std::path::PathBuf;

use super::mod_list;

pub struct Settings {
  dirty: bool,
  pub root_dir: Option<PathBuf>,
  pub new_dir: Option<String>,
  path_input_state: text_input::State,
  browse_button: button::State,
  copyright_button: button::State
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
  InitRoot(Option<PathBuf>),
  Close,
  PathChanged(String),
  OpenNativeFilePick,
  OpenNativeMessage
}

impl Settings {
  pub fn new() -> Self {
    Settings {
      dirty: true,
      root_dir: None,
      new_dir: None,
      path_input_state: text_input::State::new(),
      browse_button: button::State::new(),
      copyright_button: button::State::new()
    }
  }

  pub fn update(&mut self, message: SettingsMessage) -> Command<SettingsMessage> {
    match message {
      SettingsMessage::InitRoot(mut _root_dir) => {
        self.root_dir = _root_dir.take();
        self.dirty = false;
        return Command::none();
      },
      SettingsMessage::Close => {
        let some_path = PathBuf::from(self.new_dir.as_deref().unwrap_or_else(|| ""));

        if (*some_path).exists() {
          self.root_dir.replace(some_path);
        } else {
          self.new_dir = None;
        }

        return Command::none();
      },
      SettingsMessage::PathChanged(path) => {
        if !self.dirty {
          self.new_dir.replace(path);
        }
        return Command::none();
      },
      SettingsMessage::OpenNativeFilePick => {
        let diag = FileDialog::new().set_location("~/Desktop");

        if let Ok(some_path) = diag.show_open_single_dir() {
          if let Some(ref path_buf) = some_path {
            self.new_dir = Some(path_buf.to_string_lossy().into_owned())
          }
          
          self.root_dir = some_path;
        }

        return Command::none();
      },
      SettingsMessage::OpenNativeMessage => {
        mod_list::ModList::make_alert(COPYRIGHT.to_string());

        Command::none()
      }
    }
  }

  pub fn view(&mut self) -> Element<SettingsMessage> {
    let tmp;
    let input = TextInput::new(
      &mut self.path_input_state,
      if self.dirty { "Loading - Please wait" } else { "/" },
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
    .on_press(SettingsMessage::OpenNativeFilePick);
  
    Column::new()
      .push(
        Container::new(
          Row::new()
            .push(Text::new("Starsector Install Dir: "))
            .push(input)
            .push(browse)
            .width(Length::Fill)
            .align_items(Align::Center)
        )
        .center_y()
        .height(Length::Fill)
      )
      .push::<Element<SettingsMessage>>(
        Row::new()
          .push(Space::with_width(Length::Fill))
          .push(
            Button::new(
              &mut self.copyright_button,
              Text::new("Licensing")
            )
            .on_press(SettingsMessage::OpenNativeMessage)
          )
          .into()
      )
      .padding(5)
      .into()
  }
}

const COPYRIGHT: &str = r#"
This program is provided under the following terms:

Copyright (c) 2021 Iain Laird

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

This program makes use of multiple open source components and framewords. They include, and are not limited to:

iced, serde, tokio, infer, native-dialog, serde_json, json5, json_comments, and if_chain.
"#;

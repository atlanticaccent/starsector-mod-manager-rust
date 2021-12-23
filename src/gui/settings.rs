use iced::{
  Align, Button, Length, Text, TextInput, Command, Row, Column, Element,
  text_input, button, Container, Space, Checkbox, PickList, pick_list
};
use tinyfiledialogs as tfd;
use std::path::PathBuf;
use directories::UserDirs;

pub mod vmparams;

pub struct Settings {
  dirty: bool,
  pub root_dir: Option<PathBuf>,
  pub new_dir: Option<String>,
  path_input_state: text_input::State,
  browse_button: button::State,
  copyright_button: button::State,
  pub vmparams: Option<vmparams::VMParams>,
  vmparams_editing_enabled: bool,
  min_ram_input_state: text_input::State,
  max_ram_input_state: text_input::State,
  min_ram_pick_state: pick_list::State<vmparams::Unit>,
  max_ram_pick_state: pick_list::State<vmparams::Unit>,
  manager_update_url: bool,
  manager_update_button_state: button::State
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
  InitRoot(Option<PathBuf>),
  Close,
  PathChanged(String),
  OpenNativeFilePick,
  OpenNativeMessage,
  InitVMParams(Option<vmparams::VMParams>),
  VMParamsEditingToggled(bool),
  VMParamChanged(String, VMParamChanged),
  UnitChanged(vmparams::Unit, VMParamChanged),
  InitUpdateStatus(bool),
  OpenReleases
}

#[derive(Debug, Clone)]
pub enum VMParamChanged {
  MinRam,
  MaxRam,
  _StackThread
}

impl Settings {
  pub fn new() -> Self {
    Settings {
      dirty: true,
      root_dir: None,
      new_dir: None,
      path_input_state: text_input::State::new(),
      browse_button: button::State::new(),
      copyright_button: button::State::new(),
      vmparams: None,
      vmparams_editing_enabled: false,
      min_ram_input_state: text_input::State::new(),
      max_ram_input_state: text_input::State::new(),
      min_ram_pick_state: pick_list::State::default(),
      max_ram_pick_state: pick_list::State::default(),
      manager_update_url: false,
      manager_update_button_state: button::State::new(),
    }
  }

  pub fn update(&mut self, message: SettingsMessage) -> Command<SettingsMessage> {
    match message {
      SettingsMessage::InitUpdateStatus(status) => {
        self.manager_update_url = status;

        Command::none()
      },
      SettingsMessage::OpenReleases => {
        Command::none()
      },
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
        let maybe_path = if cfg!(target_os = "macos") {
          tfd::open_file_dialog("Select Starsector app:", UserDirs::new().unwrap().document_dir().unwrap().to_str().unwrap(), Some((&["*.app"], "*.app")))
        } else {
          tfd::select_folder_dialog("Select Starsector installation:", UserDirs::new().unwrap().document_dir().unwrap().to_str().unwrap())
        };
        if let Some(ref path) = maybe_path { 
          self.new_dir = Some(path.to_string());
        }
        self.root_dir = maybe_path.map(PathBuf::from);

        return Command::none();
      },
      SettingsMessage::OpenNativeMessage => {
        tfd::message_box_ok("Copyright:", COPYRIGHT, tfd::MessageBoxIcon::Info);

        Command::none()
      },
      SettingsMessage::VMParamsEditingToggled(toggled) => {
        self.vmparams_editing_enabled = toggled;

        Command::none()
      },
      SettingsMessage::VMParamChanged(input, kind) => {
        let current_or_zero = |current: i32| -> i32 {
          if input.len() == 0 {
            return 0
          } else {
            current
          }
        };

        let input_as_int = input.parse::<i32>();
        if let Some(mut params) = self.vmparams.as_mut() {
          match kind {
            VMParamChanged::MinRam => {
              params.heap_init.amount = input_as_int.unwrap_or(current_or_zero(params.heap_init.amount));
            },
            VMParamChanged::MaxRam => {
              params.heap_max.amount = input_as_int.unwrap_or(current_or_zero(params.heap_max.amount));
            },
            VMParamChanged::_StackThread => {
              params.thread_stack_size.amount = input_as_int.unwrap_or(current_or_zero(params.thread_stack_size.amount));
            }
          }
        }

        Command::none()
      },
      SettingsMessage::InitVMParams(mut maybe_params) => {
        self.vmparams = maybe_params.take();

        Command::none()
      },
      SettingsMessage::UnitChanged(unit, kind) => {
        if let Some(vmparams) = self.vmparams.as_mut() {
          match kind {
            VMParamChanged::MinRam => vmparams.heap_init.unit = unit,
            VMParamChanged::MaxRam => vmparams.heap_max.unit = unit,
            VMParamChanged::_StackThread => vmparams.thread_stack_size.unit = unit
          }
        }

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
      SettingsMessage::PathChanged
    )
    .padding(5);
  
    let browse = Button::new(
      &mut self.browse_button,
      Text::new("Browse ...")
    )
    .on_press(SettingsMessage::OpenNativeFilePick);
  
    let mut controls: Vec<Element<SettingsMessage>> = vec![
      Row::new()
        .push({
          if cfg!(target_os = "macos") {
            Text::new("Starsector App: ")
          } else {
            Text::new("Starsector Install Dir: ")
          }
        })
        .push(input)
        .push(browse)
        .width(Length::Fill)
        .align_items(Align::Center)
        .padding(2)
        .into(),
      Row::new()
        .push(Checkbox::new(
          self.vmparams_editing_enabled,
          "Enable VM params editing",
          SettingsMessage::VMParamsEditingToggled
        ))
        .padding(2)
        .into()
    ];

    if self.vmparams_editing_enabled {
      if let Some(vmparams) = &self.vmparams {
        controls.push(
          Row::new()
            .push(Text::new("Minimum RAM: "))
            .push(
              TextInput::new(
                &mut self.min_ram_input_state,
                "",
                &vmparams.heap_init.amount.to_string(),
                |input| -> SettingsMessage {
                  SettingsMessage::VMParamChanged(input, VMParamChanged::MinRam)
                }
              )
              .padding(5)
              .width(Length::FillPortion(2))
            )
            .push(PickList::new(
              &mut self.min_ram_pick_state,
              &vmparams::Unit::ALL[..],
              Some(vmparams.heap_init.unit),
              |unit| -> SettingsMessage {
                SettingsMessage::UnitChanged(unit, VMParamChanged::MinRam)
              }
            ))
            .push(Space::with_width(Length::Units(10)))
            .push(Text::new("Maximum RAM: "))
            .push(
              TextInput::new(
                &mut self.max_ram_input_state,
                "",
                &vmparams.heap_max.amount.to_string(),
                |input| -> SettingsMessage {
                  SettingsMessage::VMParamChanged(input, VMParamChanged::MaxRam)
                }
              )
              .padding(5)
              .width(Length::FillPortion(2))
            )
            .push(PickList::new(
              &mut self.max_ram_pick_state,
              &vmparams::Unit::ALL[..],
              Some(vmparams.heap_max.unit),
              |unit| -> SettingsMessage {
                SettingsMessage::UnitChanged(unit, VMParamChanged::MaxRam)
              }
            ))
            .push(Space::with_width(Length::FillPortion(20)))
            .align_items(Align::Center)
            .padding(2)
            .into()
        );
      } else {
        controls.push(Container::new(Text::new("VMParams editing is currently unavailable. Have you selected/saved a Starsector installation directory?")).padding(7).into())
      }
    } else {
      controls.push(Container::new(Text::new(" ")).padding(7).into())
    }

    Column::new()
      .push(
        Container::new(
          Column::with_children(controls)
        )
        .center_y()
        .height(Length::FillPortion(1))
      )
      .push(if self.manager_update_url {
        Row::new()
          .push(Space::with_width(Length::Fill))
          .push(Text::new("Mod Manager update available:"))
          .push(Space::with_width(Length::Units(10)))
          .push(Button::new(
            &mut self.manager_update_button_state,
            Text::new("Open in browser?")
            ).on_press(SettingsMessage::OpenReleases)
          )
          .align_items(iced::Align::Center)
      } else {
        Row::new().height(Length::Shrink)
      })
      .push(Space::with_height(Length::Units(10)))
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

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the `Software`), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED `AS IS`, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

This program makes use of multiple open source components and framewords. They include, and are not limited to:

infer, tokio, iced, iced_native, iced_aw, tinyfiledialogs, native-dialog, iced_futures, serde, serde_json, json5, json_comments, if_chain, reqwest, serde-aux, handwritten-json, unrar, opener, directories, tempfile, compress-tools, snafu, remove_dir_all, sublime_fuzzy, classfile-parser, zip, regex, lazy_static
"#;

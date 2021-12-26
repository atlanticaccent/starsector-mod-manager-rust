use std::path::PathBuf;
use iced::{Application, button, Button, Column, Command, Element, Length, Row, Text, executor, Clipboard, Container, Space, Subscription};
use iced_aw::{modal, Modal, Card};

use serde::{Serialize, Deserialize};
use serde_json;

use lazy_static::lazy_static;

lazy_static! {
  static ref JAVA_REGEX: regex::Regex = regex::Regex::new(r"java\.exe").expect("compile regex");
}

// const DEV_VERSION: &'static str = "IN_DEV";
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
use mod_list::{ModListMessage, ModEntryMessage};

#[derive(Default)]
struct ModalState {
  cancel_state: button::State,
  accept_state: button::State,
}


pub struct App {
  config: Option<Config>,
  settings_button: button::State,
  apply_button: button::State,
  settings_open: bool,
  settings: settings::Settings,
  mod_list: mod_list::ModList,
  manager_update_status: Option<Result<String, String>>,
  settings_changed: bool,
  modal_state: modal::State<ModalState>,
  starsector_running: bool,
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
  CloseModal(Option<(String, String, PathBuf)>),
  VersionLoaded(Result<String, LoadError>),
  StarsectorClosed(Result<(), String>),
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
        settings_changed: false,
        modal_state: modal::State::default(),
        starsector_running: false,
      },
      Command::batch(vec![
        Command::perform(Config::load(), Message::ConfigLoaded),
        Command::perform(App::get_latest_manager(), Message::TagsReceived)
      ])
    )
  }
  
  fn title(&self) -> String {
    String::from(format!("Starsector Mod Manager v{}", TAG))
  }
  
  fn update(
    &mut self,
    _message: Message,
    _clipboard: &mut Clipboard,
  ) -> Command<Message> {
    match _message {
      Message::StarsectorClosed(_res) => {
        self.starsector_running = false;
        self.modal_state.show(false);

        if let Err(err) = _res {
          dbg!(err);
        }

        Command::none()
      }
      Message::ConfigLoaded(res) => {
        let mut commands = vec![];
        match res {
          Ok(config) => {
            let resolution = (config.experimental_resolution.0.to_string(), config.experimental_resolution.1.to_string());

            commands.push(self.settings.update(SettingsMessage::InitRoot(config.install_dir.clone())).map(|m| Message::SettingsMessage(m)));
            commands.push(self.settings.update(SettingsMessage::GitWarnToggled(config.git_warn)).map(|m| Message::SettingsMessage(m)));
            commands.push(self.settings.update(SettingsMessage::ResolutionChanged(resolution)).map(|m| Message::SettingsMessage(m)));

            commands.push(self.mod_list.update(ModListMessage::SetRoot(config.install_dir.clone())).map(|m| Message::ModListMessage(m)));
            commands.push(self.mod_list.update(ModListMessage::SetLastBrowsed(config.last_browsed.clone())).map(|m| Message::ModListMessage(m)));
            self.mod_list.git_warn = config.git_warn;

            if let Some(install_dir) = &config.install_dir {
              commands.push(Command::perform(VMParams::load(install_dir.clone()), Message::VMParamsLoaded));
              commands.push(Command::perform(App::get_starsector_version(install_dir.clone()), Message::VersionLoaded))
            }

            self.config = Some(config);
          },
          Err(err) => {
            dbg!("{:?}", err);

            commands.push(self.settings.update(SettingsMessage::InitRoot(None)).map(|m| Message::SettingsMessage(m)));

            self.config = Some(Config {
              install_dir: None,
              last_browsed: None,
              git_warn: false,
              experimental_launch: false,
              experimental_resolution: (1280, 768)
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

        self.mod_list.git_warn = self.settings.git_warn;

        let mut commands = vec![
          self.settings.update(SettingsMessage::Close).map(|m| Message::SettingsMessage(m)),
          self.mod_list.update(ModListMessage::SetRoot(self.settings.root_dir.clone())).map(|m| Message::ModListMessage(m))
        ];

        if let Some(config) = self.config.as_mut() {
          config.install_dir = self.settings.root_dir.clone();
          config.git_warn = self.settings.git_warn;
          config.experimental_launch = self.settings.experimental_launch;
          config.experimental_resolution = self.settings.experimental_resolution;

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

              commands.push(Command::perform(vmparams.clone().save(install_dir.clone()), Message::VMParamsSaved));
              commands.push(Command::perform(App::get_starsector_version(install_dir.clone()), Message::VersionLoaded))
            } else {
              commands.push(Command::perform(VMParams::load(install_dir.clone()), Message::VMParamsLoaded))
            }
          }
        }

        Command::batch(commands)
      } 
      Message::SettingsMessage(settings_message) => {
        if let SettingsMessage::OpenNativeFilePick | SettingsMessage::PathChanged(_) | SettingsMessage::VMParamChanged(_, _) | SettingsMessage::UnitChanged(_, _) | SettingsMessage::GitWarnToggled(_) = settings_message {
          self.settings_changed = true;
        };
        if let SettingsMessage::OpenReleases = settings_message {
          self.open_releases();
        };
        if let SettingsMessage::GitWarnToggled(val) = settings_message {
          self.mod_list.git_warn = val;
        }

        self.settings.update(settings_message);
        return Command::none();
      },
      Message::ModListMessage(mod_list_message) => {
        let mut commands = vec![self.mod_list.update(mod_list_message.clone()).map(|m| Message::ModListMessage(m))];

        match mod_list_message {
          ModListMessage::LaunchStarsector => {
            if let Some(install_dir) = self.settings.root_dir.clone() {
              self.modal_state.show(true);
              self.starsector_running = true;
              let experimental_launch = self.settings.experimental_launch;
              let resolution = self.settings.experimental_resolution;
              commands.push(Command::perform((async move || -> Result<(), String> {
                use tokio::fs::read_to_string;
    
                let child = if experimental_launch {
                  // let mut args_raw = String::from(r"java.exe -XX:CompilerThreadPriority=1 -XX:+CompilerThreadHintNoPreempt -XX:+DisableExplicitGC -XX:+UnlockExperimentalVMOptions -XX:+AggressiveOpts -XX:+TieredCompilation -XX:+UseG1GC -XX:InitialHeapSize=2048m -XX:MaxMetaspaceSize=2048m -XX:MaxNewSize=2048m -XX:+ParallelRefProcEnabled -XX:G1NewSizePercent=5 -XX:G1MaxNewSizePercent=10 -XX:G1ReservePercent=5 -XX:G1MixedGCLiveThresholdPercent=70 -XX:InitiatingHeapOccupancyPercent=90 -XX:G1HeapWastePercent=5 -XX:MaxGCPauseMillis=50 -XX:G1HeapRegionSize=2M -XX:+UseStringDeduplication -Djava.library.path=native\windows -Xms1536m -Xmx1536m -Xss2048k -classpath janino.jar;commons-compiler.jar;commons-compiler-jdk.jar;starfarer.api.jar;starfarer_obf.jar;jogg-0.0.7.jar;jorbis-0.0.15.jar;json.jar;lwjgl.jar;jinput.jar;log4j-1.2.9.jar;lwjgl_util.jar;fs.sound_obf.jar;fs.common_obf.jar;xstream-1.4.10.jar -Dcom.fs.starfarer.settings.paths.saves=..\\saves -Dcom.fs.starfarer.settings.paths.screenshots=..\\screenshots -Dcom.fs.starfarer.settings.paths.mods=..\\mods -Dcom.fs.starfarer.settings.paths.logs=. com.fs.starfarer.StarfarerLauncher");
                  let mut args_raw = read_to_string(install_dir.join("vmparams")).await.map_err(|err| err.to_string())?;
                  args_raw = JAVA_REGEX.replace(&args_raw, "").to_string();
                  let args: Vec<&str> = args_raw.split_ascii_whitespace().collect();
      
                  std::process::Command::new(install_dir.join("jre").join("bin").join("java.exe"))
                    .current_dir(install_dir.join("starsector-core"))
                    .args(["-DlaunchDirect=true", &format!("-DstartRes={}x{}", resolution.0, resolution.1), "-DstartFS=false", "-DstartSound=true"])
                    .args(args)
                    .spawn()
                    .expect("Execute Starsector")
                } else {
                  std::process::Command::new(install_dir.join("starsector.exe"))
                    .current_dir(install_dir)
                    .spawn()
                    .expect("Execute Starsector")
                };

                child.wait_with_output().map_or_else(|err| Err(err.to_string()), |_| Ok(()))
              })(), Message::StarsectorClosed));
            } else {
              util::notif("Can't launch Starsector. Have you set the Starsector install/app path in settings?")
            };
          }
          ModListMessage::ModEntryMessage(_, ModEntryMessage::AutoUpdate) => {
            self.modal_state.show(true);
          }
          _ => {}
        }

        if let Some(config) = self.config.as_mut() {
          config.last_browsed = self.mod_list.last_browsed.clone();

          commands.push(Command::perform(config.clone().save(), Message::ConfigSaved));
        }

        Command::batch(commands)
      },
      Message::TagsReceived(res) => {
        if let Ok(tags) = &res {
          if tags > &format!("v{}", TAG) {
            self.modal_state.show(true);
            self.settings.update(SettingsMessage::InitUpdateStatus(true));
          }
        }
        self.manager_update_status = Some(res);

        Command::none()
      },
      Message::OpenReleases => {
        self.open_releases();
        
        Command::none()
      },
      Message::CloseModal(result) => {
        self.modal_state.show(false);
        if self.manager_update_status.is_some() {
          self.manager_update_status = None;
        }

        if let Some((url, target_version, old_path)) = result {
          self.mod_list.update(ModListMessage::InstallPressed(mod_list::InstallOptions::FromDownload(url, target_version, old_path))).map(|m| Message::ModListMessage(m))
        } else {
          Command::none()
        }
      },
      Message::VersionLoaded(res) => {
        if let Ok(version) = res {
          println!("Version: {}", version);
          self.mod_list.update(ModListMessage::SetVersion(version)).map(|m| Message::ModListMessage(m))
        } else {
          println!("{:?}", res);

          Command::none()
        }
      }
    }
  }
  
  fn view(&mut self) -> Element<Message> {
    let mut buttons: Row<Message> = Row::new()
      .push(Space::with_width(Length::Units(5)));

    let starsector_version = Container::new::<Element<Message>>(if let Some(version) = self.mod_list.get_game_version() {
      Text::new(format!("Starsector Version:   {}", version)).into()
    } else {
      Space::with_width(Length::Shrink).into()
    }).align_x(iced::Align::Center);
    // let versions = Column::with_children(vec![
    //   update.into(),
    //   Space::with_height(Length::Units(2)).into(),
    //   starsector_version.into()
    // ]).align_items(iced::Align::Center).padding(5).width(Length::Fill);
    buttons = if self.settings_open {
      buttons
        .push(Space::with_width(Length::FillPortion(1)))
        .push(starsector_version)
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
        .push(starsector_version)
        .push(Space::with_width(Length::FillPortion(1)))
    };

    let menu = Container::new(buttons.align_items(iced::Align::Center))
      .style(style::nav_bar::Container)
      .width(Length::Fill);

    let content: Element<Message> = if self.settings_open {
      self.settings.view().map(move |_message| {
        Message::SettingsMessage(_message)
      })
    } else {
      let entry = self.mod_list.mod_description.mod_entry.clone();

      let inner_content = self.mod_list.view().map(move |_message| {
        Message::ModListMessage(_message)
      });

      let tag = format!("v{}", TAG);
      match &self.manager_update_status {
        Some(Ok(remote)) if remote > &tag => {
          Modal::new(
            &mut self.modal_state,
            inner_content,
            move |state| {
              Card::new(
                Text::new("Mod Manager update available"),
                Column::with_children(vec![
                  Text::new("An update is available for the Mod Manager.").into(),
                  Text::new(format!("Current version is {}", TAG)).into(),
                  Text::new(format!("New version is {}", remote)).into(),
                  Text::new("Would you like to open the update in your browser?").into(),
                  Text::new("NOTE:").into(),
                  Text::new("This will not update the manager automatically, it will simply open a browser so that you can download the update yourself.").into(),
                ])
              )
              .foot(
                Column::with_children(vec![
                  iced::Rule::horizontal(2).style(style::max_rule::Rule).into(),
                  Space::with_height(Length::Units(2)).into(),
                  Row::new()
                    .spacing(10)
                    .padding(5)
                    .width(Length::Fill)
                    .push(
                      Button::new(
                        &mut state.cancel_state,
                        Text::new("Cancel"),
                      )
                      .width(Length::Fill)
                      .on_press(Message::CloseModal(None)),
                    )
                    .push(
                      Button::new(
                        &mut state.accept_state,
                        Text::new("Ok"),
                      )
                      .width(Length::Fill)
                      .on_press(Message::OpenReleases),
                    )
                    .into(),
                ])
              )
              .max_width(300)
              .on_close(Message::CloseModal(None))
              .into()
            }
          )
          .backdrop(Message::CloseModal(None))
          .on_esc(Message::CloseModal(None))
          .into()
        },
        _ if self.starsector_running => {
          Modal::new(
            &mut self.modal_state,
            inner_content,
            move |_| {
              Card::new(
                Text::new("Starsector running!"),
                Text::new("App suspended until Starsector quits.").height(Length::Fill).vertical_alignment(iced::VerticalAlignment::Center),
              )
              .max_height(200)
              .max_width(300)
              .into()
            }
          )
          .into()
        },
        _ => {
          Modal::new(
            &mut self.modal_state,
            inner_content,
            move |state| {
              Card::new(
                Text::new("Auto-update?"),
                Column::with_children(vec![
                  Text::new(format!("Do you want to automatically download and update {} from version {} to version {}?", if let Some(highlighted) = &entry {
                    &highlighted.name
                  } else {
                    "{Error: Failed to retrieve mod name}"
                  }, if let Some(current) = &entry.as_ref().map(|entry| entry.version.to_string()) {
                    current
                  } else {
                    "{Error: Failed to retrieve version}"
                  }, if let Some(remote) = &entry.as_ref().and_then(|entry| entry.remote_version.as_ref()).map(|m| m.version.to_string()) {
                    remote
                  } else {
                    "{Error: Failed to retrieve remote version}"
                  })).into(),
                  Text::new("WARNING:").color(iced::Color::from_rgb8(0xB0, 0x00, 0x20)).into(),
                  Text::new("Save compatibility is not guaranteed when updating a mod. Your save may no longer load if you apply this update.").into(),
                  Text::new("Bug reports about saves broken by using this feature will be ignored.").into(),
                ]),
              )
              .foot(
                Column::with_children(vec![
                  iced::Rule::horizontal(2).style(style::max_rule::Rule).into(),
                  Text::new("Are you sure you want to continue?").into(),
                  Space::with_height(Length::Units(2)).into(),
                  Row::new()
                    .spacing(10)
                    .padding(5)
                    .width(Length::Fill)
                    .push(
                      Button::new(
                        &mut state.cancel_state,
                        Text::new("Cancel"),
                      )
                      .width(Length::Fill)
                      .on_press(Message::CloseModal(None)),
                    )
                    .push(
                      Button::new(
                        &mut state.accept_state,
                        Text::new("Ok"),
                      )
                      .width(Length::Fill)
                      .on_press(Message::CloseModal(if let Some(highlighted) = entry.as_ref()
                        .and_then(|e| e.get_master_version())
                        .and_then(|v| v.direct_download_url.clone())
                        .zip(entry.as_ref()
                          .and_then(|entry| entry.remote_version.as_ref())
                          .map(|m| m.version.to_string())
                        )
                        .zip_with(entry.as_ref().map(|e| e.path.clone()), |(url, version): (String, String), other| (url, version, other))
                      {
                        Some(highlighted)
                      } else { None })),
                    )
                    .into(),
                ])
              )
              .max_width(300)
              .on_close(Message::CloseModal(None))
              .into()
            }
          )
          .backdrop(Message::CloseModal(None))
          .on_esc(Message::CloseModal(None))
          .into()
        }
      }
    };

    Column::new()
      .push(menu.height(Length::Shrink))
      .push(content)
      .width(Length::Fill)
      .into()
  }

  fn subscription(&self) -> Subscription<Message> {
    self.mod_list.subscription().map(Message::ModListMessage)
  }
}

impl App {
  fn open_releases(&mut self) {
    self.modal_state.show(false);
    self.manager_update_status = None;

    if let Err(_) = opener::open("https://github.com/atlanticaccent/starsector-mod-manager-rust/releases") {
      println!("Failed to open GitHub");
    }
  }

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

  async fn get_starsector_version(install_dir: PathBuf) -> Result<String, LoadError> {
    use std::io::Read;
    use classfile_parser::class_parser;
    use tokio::{task, fs};
    use if_chain::if_chain;
    use regex::bytes::Regex;

    let install_dir_clone = install_dir.clone();
    let res = task::spawn_blocking(move || {
      let mut zip = zip::ZipArchive::new(std::fs::File::open(install_dir_clone.join("starsector-core").join("starfarer_obf.jar")).unwrap()).unwrap();

      // println!("{:?}", zip.file_names().collect::<Vec<&str>>());
      
      let mut version_class = zip.by_name("com/fs/starfarer/Version.class").map_err(|_| LoadError::NoSuchFile)?;

      let mut buf: Vec<u8> = Vec::new();
      version_class.read_to_end(&mut buf)
        .map_err(|_| LoadError::ReadError)
        .and_then(|_| {
          class_parser(&buf).map_err(|_| LoadError::FormatError).map(|(_, class_file)| class_file)
        })
        .and_then(|class_file| {
          class_file.fields.iter().find_map(|f| {
            if_chain! {
              if let classfile_parser::constant_info::ConstantInfo::Utf8(name) =  &class_file.const_pool[(f.name_index - 1) as usize];
              if name.utf8_string == "versionOnly";
              if let Ok((_, attr)) = classfile_parser::attribute_info::constant_value_attribute_parser(&f.attributes.first().unwrap().info);
              if let classfile_parser::constant_info::ConstantInfo::Utf8(utf_const) = &class_file.const_pool[attr.constant_value_index as usize];
              then {
                return Some(utf_const.utf8_string.clone())
              } else {
                None
              }
            }
          }).ok_or_else(|| LoadError::FormatError)
        })
    }).await
    .map_err(|_| LoadError::ReadError)
    .flatten();

    if res.is_err() {
      lazy_static! {
        static ref RE: Regex = Regex::new(r"Starting Starsector (.*) launcher").unwrap();
      }
      fs::read(install_dir.join("starsector-core").join("starsector.log")).await
        .map_err(|_| LoadError::ReadError)
        .and_then(|file| {
          RE.captures(&file)
            .and_then(|captures| captures.get(1))
            .ok_or(LoadError::FormatError)
            .and_then(|m| String::from_utf8(m.as_bytes().to_vec()).map_err(|_| LoadError::FormatError))
        })
    } else {
      res
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
  last_browsed: Option<PathBuf>,
  git_warn: bool,
  experimental_launch: bool,
  experimental_resolution: (u32, u32),
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

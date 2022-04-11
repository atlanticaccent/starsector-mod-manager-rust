use std::{
  collections::VecDeque,
  fmt::Display,
  fs::File,
  io::{BufRead, BufReader, Read},
  iter::FromIterator,
  path::{Path, PathBuf},
  sync::Arc,
};

use druid::{
  widget::{Button, Checkbox, Controller, ControllerHost, Flex, Label, SizedBox, ViewSwitcher},
  Color, Data, KeyOrValue, Lens, LensExt, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as WidgetExtNursery};
use json_comments::strip_comments;
use serde::Deserialize;

use if_chain::if_chain;
use serde_aux::prelude::*;
use tap::Tap;

use crate::{
  app::{
    util::{parse_game_version, default_true, LabelExt},
    App, AppCommands, controllers::ModEntryClickController,
  },
  patch::{split::Split, tooltip::TooltipController},
};

use super::{
  mod_list::headings,
  util::{
    self, icons::*, BLUE_KEY, GREEN_KEY, ON_BLUE_KEY, ON_GREEN_KEY, ON_ORANGE_KEY, ON_RED_KEY,
    ON_YELLOW_KEY, ORANGE_KEY, RED_KEY, YELLOW_KEY,
  },
};

pub type GameVersion = (
  Option<String>,
  Option<String>,
  Option<String>,
  Option<String>,
);

#[derive(Debug, Clone, Deserialize, Data, Lens, PartialEq, Default)]
pub struct ModEntry {
  pub id: String,
  pub name: String,
  #[serde(default)]
  pub author: String,
  pub version: VersionUnion,
  description: String,
  #[serde(alias = "gameVersion")]
  raw_game_version: String,
  #[serde(skip)]
  pub game_version: GameVersion,
  #[serde(skip)]
  pub enabled: bool,
  #[serde(skip)]
  highlighted: bool,
  #[serde(skip)]
  pub version_checker: Option<ModVersionMeta>,
  #[serde(skip)]
  pub remote_version: Option<ModVersionMeta>,
  #[serde(skip)]
  pub update_status: Option<UpdateStatus>,
  #[serde(skip)]
  #[data(same_fn = "PartialEq::eq")]
  pub path: PathBuf,
  #[serde(skip)]
  #[serde(default = "default_true")]
  display: bool,
}

impl ModEntry {
  pub const UPDATE_RATIOS: Selector<(usize, f64)> = Selector::new("MOD_ENTRY_UPDATE_RATIOS");
  pub const REPLACE: Selector<Arc<ModEntry>> = Selector::new("MOD_ENTRY_REPLACE");
  pub const AUTO_UPDATE: Selector<Arc<ModEntry>> = Selector::new("mod_list.update.auto");
  pub const ASK_DELETE_MOD: Selector<Arc<ModEntry>> = Selector::new("mod_entry.delete");

  pub fn from_file(path: &Path) -> Result<ModEntry, ModEntryError> {
    if let Ok(mod_info_file) = std::fs::read_to_string(path.join("mod_info.json")) {
      if_chain! {
        let mut stripped = String::new();
        if strip_comments(mod_info_file.as_bytes()).read_to_string(&mut stripped).is_ok();
        if let Ok(mut mod_info) = json5::from_str::<ModEntry>(&stripped);
        then {
          mod_info.version_checker = ModEntry::parse_version_checker(path, &mod_info.id);
          mod_info.path = path.to_path_buf();
          mod_info.game_version = parse_game_version(&mod_info.raw_game_version);
          Ok(mod_info)
        } else {
          Err(ModEntryError::ParseError)
        }
      }
    } else {
      Err(ModEntryError::FileError)
    }
  }

  fn parse_version_checker(path: &Path, id: &str) -> Option<ModVersionMeta> {
    if_chain! {
      if let Ok(version_loc_file) = File::open(path.join("data").join("config").join("version").join("version_files.csv"));
      let mut lines = BufReader::new(version_loc_file).lines();
      if let Some(Ok(version_filename)) = lines.nth(1);
      if let Ok(version_data) = std::fs::read_to_string(path.join(version_filename));
      let mut no_comments = String::new();
      if strip_comments(version_data.as_bytes()).read_to_string(&mut no_comments).is_ok();
      if let Ok(normalized) = handwritten_json::normalize(&no_comments);
      if let Ok(mut version) = json5::from_str::<ModVersionMeta>(&normalized);
      then {
        version.id = id.to_string();
        Some(version)
      } else {
        None
      }
    }
  }

  pub fn set_enabled(&mut self, enabled: bool) {
    self.enabled = enabled;
  }

  pub fn ui_builder() -> impl Widget<Arc<Self>> {
    let children: VecDeque<SizedBox<Arc<ModEntry>>> = VecDeque::from_iter(vec![
      Label::wrapped_func(|text: &String, _| text.to_string())
        .lens(ModEntry::name.in_arc())
        .padding(5.)
        .expand_width(),
      Label::wrapped_func(|text: &String, _| text.to_string())
        .lens(ModEntry::id.in_arc())
        .padding(5.)
        .expand_width(),
      Label::wrapped_func(|text: &String, _| text.to_string())
        .lens(ModEntry::author.in_arc())
        .padding(5.)
        .expand_width(),
      ViewSwitcher::new(
        |entry: &Arc<ModEntry>, _| {
          entry.clone()
        },
        |data, _, env| {
          let color = data
            .update_status
            .as_ref()
            .map(|s| s.as_text_colour())
            .unwrap_or_else(|| <KeyOrValue<Color>>::from(druid::theme::TEXT_COLOR));
          Box::new(
            Flex::row()
              .with_child(Label::wrapped(&data.version.to_string()).with_text_color(color.clone()))
              .with_flex_spacer(1.)
              .tap_mut(|row| {
                let mut icon_row = Flex::row();
                let mut iter = 0;

                match data.update_status.as_ref() {
                  Some(UpdateStatus::Major(_)) => iter = 3,
                  Some(UpdateStatus::Minor(_)) => iter = 2,
                  Some(UpdateStatus::Patch(_)) => iter = 1,
                  Some(UpdateStatus::Error) => icon_row.add_child(Icon::new(REPORT)),
                  Some(UpdateStatus::Discrepancy(_)) => icon_row.add_child(Icon::new(HELP)),
                  Some(UpdateStatus::UpToDate) => icon_row.add_child(Icon::new(VERIFIED)),
                  _ => {}
                };

                for _ in 0..iter {
                  icon_row.add_child(Icon::new(NEW_RELEASES))
                }

                if let Some(update_status) = &data.update_status {
                  let tooltip = update_status.to_string();
                  let text_color = color.clone();
                  let background_color = <KeyOrValue<Color>>::from(update_status).resolve(env);
                  row.add_child(icon_row.controller(TooltipController::new(move || {
                    Label::new(tooltip.clone())
                      .with_text_color(text_color.clone())
                      .padding(5.)
                      .background(background_color.clone())
                      .border(text_color.clone(), 2.)
                      .boxed()
                  })))
                } else {
                  row.add_child(icon_row)
                }
              }),
          )
        },
      )
      .padding(5.)
      .expand_width(),
      ViewSwitcher::new(
        |entry: &Arc<ModEntry>, _| {
          if entry.version_checker.is_some()
            && entry
              .remote_version
              .as_ref()
              .and_then(|r| r.direct_download_url.as_ref())
              .is_some()
          {
            if let Some(status) = &entry.update_status {
              return status.clone();
            }
          }

          UpdateStatus::Error
        },
        |status, _, _| match status {
          UpdateStatus::Error => Box::new(Label::wrapped("Unsupported")),
          UpdateStatus::UpToDate => Box::new(Label::wrapped("No update available")),
          _ => Box::new(
            Button::from_label(Label::wrapped("Update available!")).on_click(
              |ctx: &mut druid::EventCtx, data: &mut Arc<ModEntry>, _| {
                ctx.submit_notification(ModEntry::AUTO_UPDATE.with(data.clone()))
              },
            ),
          ),
        },
      )
      .padding(5.)
      .expand_width(),
      Label::wrapped_func(|version: &GameVersion, _| {
        util::get_quoted_version(version).unwrap_or_else(|| "".to_string())
      })
      .lens(ModEntry::game_version.in_arc())
      .padding(5.)
      .expand_width(),
    ]);

    fn recursive_split(
      idx: usize,
      mut widgets: VecDeque<SizedBox<Arc<ModEntry>>>,
      ratios: &[f64],
    ) -> ControllerHost<Split<Arc<ModEntry>>, RowController> {
      if widgets.len() > 2 {
        Split::columns(
          widgets
            .pop_front()
            .expect("This better work..")
            .padding((0., 5., 0., 5.)),
          recursive_split(idx + 1, widgets, ratios),
        )
      } else {
        Split::columns(
          widgets
            .pop_front()
            .expect("This better work")
            .padding((0., 5., 0., 5.)),
          widgets
            .pop_front()
            .expect("This better work")
            .padding((0., 5., 0., 5.)),
        )
      }
      .split_point(ratios[idx])
      .bar_size(0.)
      .controller(RowController::new(idx))
    }

    Split::columns(
      Checkbox::new("")
        .lens(ModEntry::enabled.in_arc())
        .center()
        .padding(5.)
        .expand_width()
        .on_change(|ctx, _old, data, _| ctx.submit_command(ModEntry::REPLACE.with(data.clone()))),
      recursive_split(0, children, &headings::RATIOS),
    )
    .split_point(headings::ENABLED_RATIO)
    .on_click(
      |ctx: &mut druid::EventCtx, data: &mut Arc<ModEntry>, _env: &druid::Env| {
        ctx.submit_command(App::SELECTOR.with(AppCommands::UpdateModDescription(data.clone())))
      },
    )
    .controller(ModEntryClickController)
  }

  /// Set the mod entry's path.
  pub fn set_path(&mut self, path: PathBuf) {
    self.path = path;
  }
}

struct RowController {
  id: usize,
}

impl RowController {
  fn new(id: usize) -> Self {
    Self { id }
  }
}

impl Controller<Arc<ModEntry>, Split<Arc<ModEntry>>> for RowController {
  fn event(
    &mut self,
    child: &mut Split<Arc<ModEntry>>,
    ctx: &mut druid::EventCtx,
    event: &druid::Event,
    data: &mut Arc<ModEntry>,
    env: &druid::Env,
  ) {
    if let druid::Event::Command(cmd) = event {
      if let Some((idx, ratio)) = cmd.get(ModEntry::UPDATE_RATIOS) {
        if self.id == *idx {
          child.set_split_point_chosen(*ratio);
          ctx.request_layout();
        }
      }
    }

    child.event(ctx, event, data, env)
  }
}

#[derive(Debug, Clone, Deserialize, Data, PartialEq, Eq, PartialOrd, Ord)]
#[serde(untagged)]
pub enum VersionUnion {
  String(String),
  Object(Version),
}

impl Display for VersionUnion {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    let output: String = match self {
      VersionUnion::String(s) => s.to_string(),
      VersionUnion::Object(o) => o.to_string(),
    };
    write!(f, "{}", output)
  }
}

impl From<VersionUnion> for String {
  fn from(version_union: VersionUnion) -> Self {
    version_union.to_string()
  }
}

impl Default for VersionUnion {
  fn default() -> Self {
    Self::String(String::default())
  }
}

pub enum ModEntryError {
  ParseError,
  FileError,
}

#[derive(Debug, Clone, Deserialize, Eq, Data, Lens)]
pub struct ModVersionMeta {
  #[serde(alias = "masterVersionFile")]
  pub remote_url: String,
  #[serde(alias = "directDownloadURL")]
  #[serde(default)]
  pub direct_download_url: Option<String>,
  #[serde(alias = "modName")]
  pub id: String,
  #[serde(alias = "modThreadId")]
  #[serde(deserialize_with = "deserialize_string_from_number")]
  #[serde(default)]
  pub fractal_id: String,
  #[serde(alias = "modNexusId")]
  #[serde(deserialize_with = "deserialize_string_from_number")]
  #[serde(default)]
  pub nexus_id: String,
  #[serde(alias = "modVersion")]
  pub version: Version,
}

impl PartialEq for ModVersionMeta {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.version == other.version
  }
}

impl PartialOrd for ModVersionMeta {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    self.version.partial_cmp(&other.version)
  }
}

impl Ord for ModVersionMeta {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.partial_cmp(other).unwrap()
  }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord, Data, Lens)]
pub struct Version {
  #[serde(deserialize_with = "deserialize_number_from_string")]
  pub major: i32,
  #[serde(deserialize_with = "deserialize_number_from_string")]
  pub minor: i32,
  #[serde(default)]
  #[serde(deserialize_with = "deserialize_string_from_number")]
  pub patch: String,
}

impl Display for Version {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    if !self.patch.is_empty() {
      write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    } else {
      write!(f, "{}.{}", self.major, self.minor)
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Data)]
pub enum UpdateStatus {
  Error,
  UpToDate,
  Discrepancy(Version),
  Patch(Version),
  Minor(Version),
  Major(Version),
}

impl Display for UpdateStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    match self {
      UpdateStatus::Major(remote) => write!(f, "Major update available: {}", remote),
      UpdateStatus::Minor(remote) => write!(f, "Minor update available: {}", remote),
      UpdateStatus::Patch(remote) => write!(f, "Patch available: {}", remote),
      UpdateStatus::UpToDate => write!(f, "Up to date"),
      UpdateStatus::Error => write!(f, "Error"),
      UpdateStatus::Discrepancy(_) => write!(f, "Discrepancy"),
    }
  }
}

impl From<(&ModVersionMeta, &Option<ModVersionMeta>)> for UpdateStatus {
  fn from((local, remote): (&ModVersionMeta, &Option<ModVersionMeta>)) -> Self {
    if let Some(remote) = remote {
      let local = &local.version;
      let remote = remote.version.clone();

      if remote == *local {
        UpdateStatus::UpToDate
      } else if remote < *local {
        UpdateStatus::Discrepancy(remote)
      } else if remote.major - local.major > 0 {
        UpdateStatus::Major(remote)
      } else if remote.minor - local.minor > 0 {
        UpdateStatus::Minor(remote)
      } else {
        UpdateStatus::Patch(remote)
      }
    } else {
      UpdateStatus::Error
    }
  }
}

impl From<&UpdateStatus> for KeyOrValue<Color> {
  fn from(status: &UpdateStatus) -> Self {
    match status {
      UpdateStatus::Major(_) => ORANGE_KEY.into(),
      UpdateStatus::Minor(_) => YELLOW_KEY.into(),
      UpdateStatus::Patch(_) => BLUE_KEY.into(),
      UpdateStatus::Discrepancy(_) => Color::from_hex_str("810181").unwrap().into(),
      UpdateStatus::Error => RED_KEY.into(),
      UpdateStatus::UpToDate => GREEN_KEY.into(),
    }
  }
}

impl UpdateStatus {
  fn as_text_colour(&self) -> KeyOrValue<Color> {
    match self {
      UpdateStatus::Major(_) => ON_ORANGE_KEY.into(),
      UpdateStatus::Minor(_) => ON_YELLOW_KEY.into(),
      UpdateStatus::Patch(_) => ON_BLUE_KEY.into(),
      UpdateStatus::Discrepancy(_) => Color::from_hex_str("ffd6f7").unwrap().into(),
      UpdateStatus::Error => ON_RED_KEY.into(),
      UpdateStatus::UpToDate => ON_GREEN_KEY.into(),
    }
  }
}

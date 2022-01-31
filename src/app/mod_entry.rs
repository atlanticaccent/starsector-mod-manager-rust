use std::{
  io::{Read, BufReader, BufRead},
  path::PathBuf,
  fs::File,
  fmt::Display, collections::VecDeque, iter::FromIterator, sync::Arc
};

use druid::{widget::{Checkbox, Label, LineBreaking, SizedBox, Controller, ControllerHost}, WidgetExt, Lens, Data, Widget, Selector, LensExt};
use druid_widget_nursery::WidgetExt as WidgetExtNursery;
use serde::Deserialize;
use json_comments::strip_comments;
use json5;
use handwritten_json;
use if_chain::if_chain;
use serde_aux::prelude::*;
use sublime_fuzzy::best_match;
use lazy_static::lazy_static;
use regex::Regex;

use crate::{app::{App, AppCommands}, patch::split::Split};

use super::{util, mod_list::headings};

lazy_static! {
  static ref VERSION_REGEX: Regex = Regex::new(r"\.|a-RC|A-RC|a-rc|a").unwrap();
}

type GameVersion = (Option<String>, Option<String>, Option<String>, Option<String>);

#[derive(Debug, Clone, Deserialize, Data, Lens, PartialEq, Default)]
pub struct ModEntry {
  pub id: String,
  pub name: String,
  #[serde(default)]
  author: String,
  pub version: VersionUnion,
  description: String,
  #[serde(alias = "gameVersion")]
  raw_game_version: String,
  #[serde(skip)]
  game_version: GameVersion,
  #[serde(skip)]
  pub enabled: bool,
  #[serde(skip)]
  highlighted: bool,
  #[serde(skip)]
  pub version_checker: Option<ModVersionMeta>,
  #[serde(skip)]
  pub remote_version: Option<ModVersionMeta>,
  #[serde(skip)]
  update_status: Option<UpdateStatus>,
  #[serde(skip)]
  #[data(same_fn="PartialEq::eq")]
  pub path: PathBuf,
  #[serde(skip)]
  #[serde(default = "ModEntry::def_true")]
  display: bool,
  #[serde(skip)]
  search_score: Option<isize>,
}

pub enum EntryCommands {
  UpdateRatios(usize, f64),
}

impl ModEntry {
  pub const UPDATE_RATIOS: Selector<EntryCommands> = Selector::new("MOD_ENTRY_UPDATE_RATIOS");
  pub const REPLACE: Selector<Arc<ModEntry>> = Selector::new("MOD_ENTRY_REPLACE");

  pub fn from_file(path: &PathBuf) -> Result<ModEntry, ModEntryError> {
    if let Ok(mod_info_file) = std::fs::read_to_string(path.join("mod_info.json")) {
      if_chain! {
        let mut stripped = String::new();
        if strip_comments(mod_info_file.as_bytes()).read_to_string(&mut stripped).is_ok();
        if let Ok(mut mod_info) = json5::from_str::<ModEntry>(&stripped);
        then {
          mod_info.version_checker = ModEntry::parse_version_checker(path, &mod_info.id);
          mod_info.path = path.clone();
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

  fn parse_version_checker(path: &PathBuf, id: &String) -> Option<ModVersionMeta> {
    if_chain! {
      if let Ok(version_loc_file) = File::open(path.join("data").join("config").join("version").join("version_files.csv"));
      let lines = BufReader::new(version_loc_file).lines();
      if let Some(Ok(version_filename)) = lines.skip(1).next();
      if let Ok(version_data) = std::fs::read_to_string(path.join(version_filename));
      let mut no_comments = String::new();
      if strip_comments(version_data.as_bytes()).read_to_string(&mut no_comments).is_ok();
      if let Ok(normalized) = handwritten_json::normalize(&no_comments);
      if let Ok(mut version) = json5::from_str::<ModVersionMeta>(&normalized);
      then {
        version.id = id.clone();
        Some(version)
      } else {
        None
      }
    }
  }

  fn def_true() -> bool { true }

  pub fn set_enabled(&mut self, enabled: bool) {
    self.enabled = enabled;
  }

  pub fn ui_builder() -> impl Widget<Arc<Self>> {
    let ratios = headings::RATIOS.lock().expect("Lock in single thread");

    let children: VecDeque<SizedBox<Arc<ModEntry>>> = VecDeque::from_iter(vec![
      Label::dynamic(|text: &String, _| text.to_string()).with_line_break_mode(LineBreaking::WordWrap).lens(ModEntry::name.in_arc()).expand_width(),
      Label::dynamic(|text: &String, _| text.to_string()).with_line_break_mode(LineBreaking::WordWrap).lens(ModEntry::id.in_arc()).expand_width(),
      Label::dynamic(|text: &String, _| text.to_string()).with_line_break_mode(LineBreaking::WordWrap).lens(ModEntry::author.in_arc()).expand_width(),
      Label::dynamic(|version: &VersionUnion, _| version.to_string()).with_line_break_mode(LineBreaking::WordWrap).lens(ModEntry::version.in_arc()).expand_width(),
      Label::dynamic(|text: &Option<ModVersionMeta>, _| text.as_ref().and_then(|r| r.direct_download_url.as_ref()).is_some().to_string()).with_line_break_mode(LineBreaking::WordWrap).lens(ModEntry::remote_version.in_arc()).expand_width(),
      Label::dynamic(|version: &GameVersion, _| util::get_game_version(version).unwrap_or("".to_string())).with_line_break_mode(LineBreaking::WordWrap).lens(ModEntry::game_version.in_arc()).expand_width()
    ]);
    
    fn recursive_split(idx: usize, mut widgets: VecDeque<SizedBox<Arc<ModEntry>>>, ratios: &[f64]) -> ControllerHost<Split<Arc<ModEntry>>, RowController> {
      if widgets.len() > 2 {
        Split::columns(
          widgets.pop_front().expect("This better work..").padding((0., 5., 0., 5.)),
          recursive_split(idx + 1, widgets, ratios)
        )
      } else {
        Split::columns(
          widgets.pop_front().expect("This better work").padding((0., 5., 0., 5.)),
          widgets.pop_front().expect("This better work").padding((0., 5., 0., 5.))
        )
      }
      .split_point(ratios[idx]).bar_size(0.).controller(RowController::new(idx))
    }
    
    Split::columns(
      Checkbox::new("").lens(ModEntry::enabled.in_arc()).center().on_change(|ctx, _old, data, _| {
        ctx.submit_command(ModEntry::REPLACE.with(data.clone()))
      }),
      recursive_split(0, children, &ratios)
    ).split_point(1. / 7.).on_click(|ctx: &mut druid::EventCtx, data: &mut Arc<ModEntry>, _env: &druid::Env| {
      ctx.submit_command(App::SELECTOR.with(AppCommands::UpdateModDescription(data.clone())))
    })
  }

  /// Set the mod entry's path.
  pub fn set_path(&mut self, path: PathBuf) {
    self.path = path;
  }
}

struct RowController {
  id: usize
}

impl RowController {
  fn new(id: usize) -> Self { Self { id } }
}

impl Controller<Arc<ModEntry>, Split<Arc<ModEntry>>> for RowController {
  fn event(&mut self, child: &mut Split<Arc<ModEntry>>, ctx: &mut druid::EventCtx, event: &druid::Event, data: &mut Arc<ModEntry>, env: &druid::Env) {
    if let druid::Event::Command(cmd) = event {
      if let Some(EntryCommands::UpdateRatios(idx, ratio)) = cmd.get(ModEntry::UPDATE_RATIOS) {
        if self.id == *idx {
          child.set_split_point_chosen(*ratio);
          ctx.request_layout();
        }
      }
    }

    child.event(ctx, event, data, env)
  }
}

  /**
   * Parses a given version into a four-tuple of the assumed components.
   * Assumptions:
   * - The first component is always EITHER 0 and thus the major component OR it has been omitted and the first component is the minor component
   * - If there are two components it is either the major and minor components OR minor and patch OR minor and RC (release candidate)
   * - If there are three components it is either the major, minor and patch OR major, minor and RC OR minor, patch and RC
   * - If there are four components then the first components MUST be 0 and MUST be the major component, and the following components 
        are the minor, patch and RC components
   */
fn parse_game_version(text: &str) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
  let components: Vec<&str> = VERSION_REGEX.split(text).filter(|c| !c.is_empty()).collect();

  match components.as_slice() {
    [major, minor] if major == &"0" => {
      // text = format!("{}.{}a", major, minor);
      (Some(major.to_string()), Some(minor.to_string()), None, None)
    }
    [minor, patch_rc] => {
      // text = format!("0.{}a-RC{}", minor, rc);
      if text.contains("a-RC") {
        (Some("0".to_string()), Some(minor.to_string()), None, Some(patch_rc.to_string()))
      } else {
        (Some("0".to_string()), Some(minor.to_string()), Some(patch_rc.to_string()), None)
      }
    }
    [major, minor, patch_rc] if major == &"0" => {
      // text = format!("{}.{}a-RC{}", major, minor, rc);
      if text.contains("a-RC") {
        (Some(major.to_string()), Some(minor.to_string()), None, Some(patch_rc.to_string()))
      } else {
        (Some(major.to_string()), Some(minor.to_string()), Some(patch_rc.to_string()), None)
      }
    }
    [minor, patch, rc] => {
      // text = format!("0.{}.{}a-RC{}", minor, patch, rc);
      (Some("0".to_string()), Some(minor.to_string()), Some(patch.to_string()), Some(rc.to_string()))
    }
    [major, minor, patch, rc] if major == &"0" => {
      // text = format!("{}.{}.{}a-RC{}", major, minor, patch, rc);
      (Some(major.to_string()), Some(minor.to_string()), Some(patch.to_string()), Some(rc.to_string()))
    }
    _ => {
      dbg!("Failed to normalise mod's quoted game version");
      (None, None, None, None)
    }
  }
}

#[derive(Debug, Clone, Deserialize, Data, PartialEq)]
#[serde(untagged)]
pub enum VersionUnion {
  String(String),
  Object(Version)
}

impl Display for VersionUnion {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    let output: String = match self {
      VersionUnion::String(s) => s.to_string(),
      VersionUnion::Object(o) => o.to_string()
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
  FileError
}

#[derive(Debug, Clone, Deserialize, Eq, Ord, Data, Lens)]
pub struct ModVersionMeta {
  #[serde(alias="masterVersionFile")]
  pub remote_url: String,
  #[serde(alias="directDownloadURL")]
  #[serde(default)]
  pub direct_download_url: Option<String>,
  #[serde(alias="modName")]
  pub id: String,
  #[serde(alias="modThreadId")]
  #[serde(deserialize_with="deserialize_string_from_number")]
  #[serde(default)]
  fractal_id: String,
  #[serde(alias="modNexusId")]
  #[serde(deserialize_with="deserialize_string_from_number")]
  #[serde(default)]
  nexus_id: String,
  #[serde(alias="modVersion")]
  pub version: Version
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

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord, Data, Lens)]
pub struct Version {
  #[serde(deserialize_with="deserialize_number_from_string")]
  pub major: i32,
  #[serde(deserialize_with="deserialize_number_from_string")]
  pub minor: i32,
  #[serde(default)]
  #[serde(deserialize_with="deserialize_string_from_number")]
  pub patch: String
}

impl Display for Version {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    if self.patch.len() > 0 {
      write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    } else {
      write!(f, "{}.{}", self.major, self.minor)
    }
  }
}


#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Data)]
pub enum UpdateStatus {
  Error,
  Major(Version),
  Minor(Version),
  Patch(Version),
  UpToDate,
  Discrepancy(Version),
}

impl Display for UpdateStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    match self {
      UpdateStatus::Major(_) => write!(f, "Major"),
      UpdateStatus::Minor(_) => write!(f, "Minor"),
      UpdateStatus::Patch(_) => write!(f, "Patch"),
      UpdateStatus::UpToDate => write!(f, "Up to date"),
      UpdateStatus::Error => write!(f, "Error"),
      UpdateStatus::Discrepancy(_) => write!(f, "Discrepancy"),
    }
  }
}


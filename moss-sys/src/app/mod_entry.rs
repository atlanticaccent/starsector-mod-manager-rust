use core::fmt;
use std::{
  borrow::{Borrow, Cow},
  fmt::Display,
  fs::File,
  hash::Hash,
  io::{BufRead, BufReader, Read},
  path::{Path, PathBuf},
};

use chrono::{DateTime, Local, Utc};
use druid::{
  kurbo::Line,
  lens, theme,
  widget::{Button, Checkbox, Either, Flex, Label, Painter, ViewSwitcher},
  Color, Data, ExtEventSink, KeyOrValue, Lens, RenderContext as _, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as _};
use fake::Dummy;
use json_comments::StripComments;
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;

use crate::{
  app::{
    app_delegate::AppCommands,
    controllers::{next_id, MaxSizeBox, SharedIdHoverState},
    mod_description::{notify_enabled, ModDescription},
    mod_list::{headings::Heading, ModList},
    util::{
      self, default_true,
      icons::{NEW_RELEASES, REPORT, SICK, THUMB_UP},
      parse_game_version, LabelExt, LensExtExt, Tap, WidgetExtEx, WithHoverIdState as _,
    },
    App, SharedFromEnv,
  },
  nav_bar::{Nav, NavLabel},
  patch::table::{FlexTable, RowData},
  theme::{
    BLUE_KEY, GREEN_KEY, ON_BLUE_KEY, ON_GREEN_KEY, ON_ORANGE_KEY, ON_RED_KEY, ON_YELLOW_KEY,
    ORANGE_KEY, RED_KEY, YELLOW_KEY,
  },
  widgets::card::Card,
  ENV_STATE,
};

pub type GameVersion = (
  Option<String>,
  Option<String>,
  Option<String>,
  Option<String>,
);

#[derive(Debug, Clone, Deserialize, Data, Lens, Default, Dummy)]
pub struct ModEntry<T = ()> {
  pub id: String,
  pub name: String,
  #[serde(default)]
  pub author: Option<String>,
  pub version: Version,
  description: String,
  #[serde(
    alias = "gameVersion",
    deserialize_with = "ModEntry::deserialize_game_version"
  )]
  pub game_version: GameVersion,
  #[serde(default, deserialize_with = "deserialize_bool_from_anything")]
  pub utility: bool,
  #[data(eq)]
  #[serde(deserialize_with = "ModEntry::deserialize_dependencies", default)]
  pub dependencies: std::sync::Arc<Vec<Dependency>>,
  #[serde(
    alias = "totalConversion",
    default,
    deserialize_with = "deserialize_bool_from_anything"
  )]
  pub total_conversion: bool,
  #[serde(skip)]
  pub enabled: bool,
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
  #[serde(skip)]
  pub manager_metadata: ModMetadata,
  #[serde(skip, default)]
  #[data(ignore)]
  pub view_state: T,
}

#[derive(Debug, Clone, PartialEq, Data, Deserialize, Dummy)]
pub struct Dependency {
  pub id: String,
  pub name: Option<String>,
  pub version: Option<Version>,
}

impl Display for Dependency {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let Dependency { id, name, version } = self;
    write!(f, "{}", if let Some(name) = name { name } else { id })?;
    if let Some(version) = version {
      write!(f, "@{version}")?;
    }

    Ok(())
  }
}

#[derive(Clone, Data, Lens)]
pub struct ViewState {
  hover_state: SharedIdHoverState,
  pub updating: bool,
  id: u64,
}

impl ViewState {
  fn new() -> Self {
    Self {
      hover_state: SharedIdHoverState::default(),
      updating: false,
      id: next_id(),
    }
  }
}

impl Default for ViewState {
  fn default() -> Self {
    Self::new()
  }
}

impl fmt::Debug for ViewState {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("ViewState").finish()
  }
}

pub type ViewModEntry = ModEntry<ViewState>;

impl<T> ModEntry<T> {
  pub fn fractal_link() -> impl Lens<Self, Option<String>> {
    Self::version_checker.compute(|v| {
      v.as_ref().map(|v| &v.fractal_id).and_then(|s| {
        (!s.is_empty()).then(|| format!("{}{}", ModDescription::FRACTAL_URL, s.clone()))
      })
    })
  }

  pub fn nexus_link() -> impl Lens<Self, Option<String>> {
    Self::version_checker.compute(|v| {
      v.as_ref().map(|v| &v.nexus_id).and_then(|s| {
        (!s.is_empty()).then(|| format!("{}{}", ModDescription::NEXUS_URL, s.clone()))
      })
    })
  }

  pub fn set_enabled(&mut self, enabled: bool) {
    self.enabled = enabled;
  }

  /// Set the mod entry's path.
  pub fn set_path(&mut self, path: PathBuf) {
    self.path = path;
  }
}

impl<T: Default> ModEntry<T> {
  pub fn from_file(path: &Path, manager_metadata: ModMetadata) -> Result<Self, ModEntryError> {
    if let Ok(mod_info_file) = std::fs::read_to_string(path.join("mod_info.json")) {
      let mut stripped = String::new();
      if StripComments::new(mod_info_file.as_bytes())
        .read_to_string(&mut stripped)
        .is_ok()
        && let Ok(mut mod_info) = json5::from_str::<Self>(&stripped)
      {
        mod_info.version_checker = ModEntry::parse_version_checker(path, &mod_info.id);
        mod_info.path = path.to_path_buf();
        mod_info.manager_metadata = manager_metadata;
        Ok(mod_info)
      } else {
        Err(ModEntryError::ParseError)
      }
    } else {
      Err(ModEntryError::FileError)
    }
  }
}

impl ModEntry {
  pub const ASK_DELETE_MOD: Selector<ModEntry> = Selector::new("mod_entry.delete");
  pub const AUTO_UPDATE: Selector<ModEntry> = Selector::new("mod_list.update.auto");
  pub const REPLACE: Selector<ModEntry> = Selector::new("MOD_ENTRY_REPLACE");

  fn parse_version_checker(path: &Path, id: &str) -> Option<ModVersionMeta> {
    let mut no_comments = String::new();
    if let Ok(version_loc_file) = File::open(
      path
        .join("data")
        .join("config")
        .join("version")
        .join("version_files.csv"),
    ) && let Some(Ok(version_filename)) = BufReader::new(version_loc_file).lines().nth(1)
      && let Some(version_filename) = version_filename.split(',').next()
      && let Ok(version_data) = std::fs::read_to_string(path.join(version_filename))
      && StripComments::new(version_data.as_bytes())
        .read_to_string(&mut no_comments)
        .is_ok()
      && let Ok(normalized) = handwritten_json::normalize(&no_comments)
      && let Ok(mut version) = json5::from_str::<ModVersionMeta>(&normalized)
    {
      version.id = id.to_string();
      Some(version)
    } else {
      None
    }
  }

  fn deserialize_game_version<'de, D>(deserializer: D) -> Result<GameVersion, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let buf = String::deserialize(deserializer)?;

    Ok(parse_game_version(&buf))
  }

  fn deserialize_dependencies<'de, D>(
    deserializer: D,
  ) -> Result<std::sync::Arc<Vec<Dependency>>, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    #[derive(Debug, Deserialize)]
    struct RawDependency {
      id: Option<String>,
      name: Option<String>,
      version: Option<Version>,
    }

    let dependencies = Vec::<RawDependency>::deserialize(deserializer)?;

    Ok(std::sync::Arc::new(
      dependencies
        .into_iter()
        .filter_map(|RawDependency { id, name, version }| {
          id.map(|id| Dependency { id, name, version })
        })
        .collect(),
    ))
  }
}

impl ViewModEntry {
  pub fn view_cell(&self, heading: Heading) -> Option<impl Widget<Self>> {
    if heading == Heading::Score {
      return None;
    }

    let cell = if heading == Heading::Enabled {
      Checkbox::new("")
        .center()
        .padding(5.)
        .lens(ViewModEntry::enabled)
        .on_change(notify_enabled)
        .boxed()
    } else {
      match heading {
        header @ (Heading::ID | Heading::Name | Heading::Author) => {
          let label = Label::wrapped_func(|text: &String, _| text.to_string());
          match header {
            Heading::ID => label.lens(ViewModEntry::id).padding(5.).expand_width(),
            Heading::Name => label.lens(ViewModEntry::name).padding(5.).expand_width(),
            Heading::Author => label
              .lens(
                ViewModEntry::author
                  .compute(|author| author.clone().unwrap_or("Unknown".to_owned())),
              )
              .padding(5.)
              .expand_width(),
            _ => unreachable!(),
          }
          .boxed()
        }
        Heading::GameVersion => Label::wrapped_func(|version: &GameVersion, _| {
          util::get_quoted_version(version).unwrap_or_default()
        })
        .lens(ViewModEntry::game_version)
        .padding(5.)
        .expand_width()
        .boxed(),
        Heading::Version => ViewModEntry::version_cell(),
        Heading::AutoUpdateSupport => Either::new(
          |entry: &ViewModEntry, _| {
            entry
              .remote_version
              .as_ref()
              .and_then(|r| r.direct_download_url.as_ref())
              .is_some()
          },
          Either::new(
            |entry: &ViewModEntry, _| {
              entry
                .update_status
                .as_ref()
                .is_some_and(|status| status != &UpdateStatus::Error)
            },
            Either::new(
              |entry: &ViewModEntry, _| {
                entry.update_status.as_ref().is_some_and(|status| {
                  !matches!(
                    status,
                    &UpdateStatus::UpToDate | &UpdateStatus::Discrepancy(_)
                  )
                })
              },
              Button::from_label(Label::wrapped("Update available!")).on_click(
                |ctx: &mut druid::EventCtx, data: &mut ViewModEntry, _| {
                  ctx.submit_notification(ModEntry::AUTO_UPDATE.with(data.clone().into()));
                },
              ),
              Label::wrapped("No update available"),
            ),
            Label::wrapped("Unsupported"),
          ),
          Label::wrapped("Unsupported"),
        )
        .padding(5.)
        .expand_width()
        .boxed(),
        Heading::InstallDate => Label::wrapped_func(|data: &ModMetadata, _| {
          if let Some(date) = data.install_date {
            DateTime::<Local>::from(date)
              .format("%v %I:%M%p")
              .to_string()
          } else {
            String::from("Unknown")
          }
        })
        .lens(ViewModEntry::manager_metadata)
        .padding(5.)
        .expand_width()
        .boxed(),
        Heading::Type => Label::wrapped_func(|data: &ModEntry<ViewState>, _| {
          if data.total_conversion {
            "Total Conversion"
          } else if data.utility {
            "Utility"
          } else {
            "Standard"
          }
        })
        .padding(5.)
        .expand_width()
        .boxed(),
        Heading::Enabled | Heading::Score => unreachable!(),
      }
      .on_click(|ctx, data, _| {
        ctx.submit_command(App::SELECTOR.with(AppCommands::UpdateModDescription(
          ModDescription::from_entry(data),
        )));
        ctx.submit_command(Nav::NAV_SELECTOR.with(NavLabel::ModDetails));
      })
      .boxed()
    };

    Some(
      cell
        .lens(lens!((ViewModEntry, SharedIdHoverState), 0))
        .padding(2.0)
        .background(ViewModEntry::cell_painter())
        .with_shared_id_hover_state_opts(self.view_state.hover_state.clone(), false),
    )
  }

  fn version_cell() -> Box<dyn Widget<ViewModEntry>> {
    Either::new(
      |data: &(Option<UpdateStatus>, Version), _| data.0.is_some(),
      ViewSwitcher::new(
        |data: &(Option<UpdateStatus>, Version), env| {
          (data.clone(), env.get(ENV_STATE).show_discrepancy)
        },
        |_, (update_status, version_union), env| {
          if let Some(update_status) = update_status {
            let update_status = if env.shared_data().show_discrepancy {
              UpdateStatus::UpToDate
            } else {
              update_status.clone()
            };
            Box::new(
              Flex::row()
                .with_child(Label::new(version_union.to_string()))
                .with_flex_spacer(1.)
                .tap(|row| {
                  let mut icon_row = Flex::row();
                  let mut iter = 0;

                  match update_status {
                    UpdateStatus::Major(_) => iter = 3,
                    UpdateStatus::Minor(_) => iter = 2,
                    UpdateStatus::Patch(_) => iter = 1,
                    UpdateStatus::Error => icon_row.add_child(Icon::new(*REPORT)),
                    UpdateStatus::Discrepancy(_) => icon_row.add_child(Icon::new(*SICK)),
                    UpdateStatus::UpToDate => icon_row.add_child(Icon::new(*THUMB_UP)),
                  };

                  for _ in 0..iter {
                    icon_row.add_child(Icon::new(*NEW_RELEASES));
                  }

                  let tooltip = match update_status {
                    UpdateStatus::Error => "Error\nThere was an error retrieving or parsing this \
                                            mod's version information."
                      .to_string(),
                    UpdateStatus::Discrepancy(_) => {
                      "\
                          Discrepancy\nThe installed version of this mod is higher than the \
                       version available from the server.\nThis usually means the mod author has \
                       forgotten to update their remote version file and is not a cause for alarm."
                        .to_string()
                    }
                    _ => update_status.to_string(),
                  };
                  let builder = Card::builder();
                  row.add_child(
                    icon_row
                      .padding(2.0)
                      .wrap_with_hover_state(false, true)
                      .stack_tooltip_custom(
                        match (&update_status).into() {
                          druid::KeyOrValue::Concrete(color) => builder.with_background(color),
                          druid::KeyOrValue::Key(key) => builder.with_background(key),
                        }
                        .build(MaxSizeBox::new(
                          Label::new(tooltip)
                            .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
                            .with_text_color(update_status.as_text_colour())
                            .padding((5.0, 0.0)),
                          druid::widget::Axis::Horizontal,
                          300.0,
                        )),
                      )
                      .with_offset((10.0, 10.0)),
                  );
                }),
            )
          } else {
            Label::dynamic(|data: &(Option<UpdateStatus>, Version), _| data.1.to_string()).boxed()
          }
        },
      ),
      Label::dynamic(|data: &(Option<UpdateStatus>, Version), _| data.1.to_string()),
    )
    .lens(
      lens::Identity
        .compute(|entry: &ViewModEntry| (entry.update_status.clone(), entry.version.clone())),
    )
    .padding(5.)
    .expand_width()
    .boxed()
  }

  fn cell_painter() -> Painter<(ModEntry<ViewState>, SharedIdHoverState)> {
    Painter::new(|ctx, data: &(ViewModEntry, SharedIdHoverState), env| {
      if data.1 .1.get() {
        let rect = ctx.size().to_rect().inset(-0.5);
        ctx.stroke(
          Line::new((rect.x0, rect.y0), (rect.x1, rect.y0)),
          &env.get(theme::BORDER_DARK),
          1.0,
        );
        ctx.stroke(
          Line::new((rect.x0, rect.y1), (rect.x1, rect.y1)),
          &env.get(theme::BORDER_DARK),
          1.0,
        );
        let column = env.get(FlexTable::<ModList>::COL_IDX);
        let total_columns = env.get(FlexTable::<ModList>::TOTAL_COLUMNS);
        if column == 0 {
          ctx.stroke(
            Line::new((rect.x0, rect.y0), (rect.x0, rect.y1)),
            &env.get(theme::BORDER_DARK),
            1.0,
          );
        }
        if column == total_columns - 1 {
          ctx.stroke(
            Line::new((rect.x1, rect.y0), (rect.x1, rect.y1)),
            &env.get(theme::BORDER_DARK),
            1.0,
          );
        }
      }
    })
  }
}

impl<T> Hash for ModEntry<T> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.id.hash(state);
    self.name.hash(state);
    self.author.hash(state);
    self.version.hash(state);
    self.description.hash(state);
    self.game_version.hash(state);
    self.enabled.hash(state);
    self.version_checker.hash(state);
    self.remote_version.hash(state);
    self.update_status.hash(state);
    self.path.hash(state);
    self.manager_metadata.hash(state);
  }
}

impl<T> PartialEq for ModEntry<T> {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.name == other.name
      && self.author == other.author
      && self.version == other.version
      && self.description == other.description
      && self.game_version == other.game_version
      && self.enabled == other.enabled
      && self.version_checker == other.version_checker
      && self.remote_version == other.remote_version
      && self.update_status == other.update_status
      && self.path == other.path
      && self.display == other.display
      && self.manager_metadata == other.manager_metadata
  }
}

impl<T> Eq for ModEntry<T> {}

impl From<ModEntry> for ViewModEntry {
  fn from(
    ModEntry {
      id,
      name,
      author,
      version,
      description,
      game_version,
      utility,
      dependencies,
      total_conversion,
      enabled,
      version_checker,
      remote_version,
      update_status,
      path,
      display,
      manager_metadata,
      view_state: (),
    }: ModEntry,
  ) -> Self {
    ViewModEntry {
      id,
      name,
      author,
      version,
      description,
      game_version,
      utility,
      dependencies,
      total_conversion,
      enabled,
      version_checker,
      remote_version,
      update_status,
      path,
      display,
      manager_metadata,
      view_state: ViewState::new(),
    }
  }
}

impl<'a> From<&'a ModEntry> for ViewModEntry {
  fn from(value: &'a ModEntry) -> Self {
    value.clone().into()
  }
}

impl From<ViewModEntry> for ModEntry {
  fn from(
    ViewModEntry {
      id,
      name,
      author,
      version,
      description,
      game_version,
      utility,
      dependencies,
      total_conversion,
      enabled,
      version_checker,
      remote_version,
      update_status,
      path,
      display,
      manager_metadata,
      view_state: _,
    }: ViewModEntry,
  ) -> Self {
    ModEntry {
      id,
      name,
      author,
      version,
      description,
      game_version,
      utility,
      dependencies,
      total_conversion,
      enabled,
      version_checker,
      remote_version,
      update_status,
      path,
      display,
      manager_metadata,
      view_state: (),
    }
  }
}

impl<'a> From<&'a ViewModEntry> for ModEntry {
  fn from(value: &'a ViewModEntry) -> Self {
    value.clone().into()
  }
}

impl RowData for ViewModEntry {
  type Column = super::mod_list::headings::Heading;
  type Id = String;

  fn id(&self) -> String {
    self.id.clone()
  }

  fn cell(&self, column: &Self::Column) -> Box<dyn Widget<ViewModEntry>> {
    self.view_cell(*column).unwrap().boxed()
  }
}

#[derive(Debug, Clone, Deserialize, Data, PartialEq, Eq, PartialOrd, Ord, Hash, Dummy)]
#[serde(untagged)]
pub enum Version {
  Simple(String),
  Complex(VersionComplex),
}

impl Version {
  pub fn major(&self) -> Cow<'_, str> {
    match self {
      Version::Simple(str) => str
        .split_once('.')
        .map(|(major, _)| major.into())
        .unwrap_or_default(),
      Version::Complex(complex) => complex.major.to_string().into(),
    }
  }
}

impl Display for Version {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    let display: &dyn Display = match self {
      Version::Simple(s) => s,
      Version::Complex(o) => o,
    };

    write!(f, "{display}")
  }
}

impl From<Version> for String {
  fn from(version_union: Version) -> Self {
    version_union.to_string()
  }
}

impl Default for Version {
  fn default() -> Self {
    Self::Simple(String::default())
  }
}

#[derive(Debug)]
pub enum ModEntryError {
  ParseError,
  FileError,
}

#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Debug, Clone, Deserialize, Eq, Data, Lens, Hash, Dummy)]
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
  pub version: VersionComplex,
}

impl PartialEq for ModVersionMeta {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.version == other.version
  }
}

impl PartialOrd for ModVersionMeta {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.version.cmp(&other.version))
  }
}

impl Ord for ModVersionMeta {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.partial_cmp(other).unwrap()
  }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord, Data, Lens, Hash, Dummy)]
pub struct VersionComplex {
  #[serde(deserialize_with = "deserialize_number_from_string")]
  pub major: i32,
  #[serde(deserialize_with = "deserialize_number_from_string")]
  pub minor: i32,
  #[serde(default)]
  #[serde(deserialize_with = "deserialize_string_from_number")]
  pub patch: String,
}

impl VersionComplex {
  pub const DUMMY: VersionComplex = VersionComplex {
    major: 0,
    minor: 0,
    patch: String::new(),
  };
}

impl Display for VersionComplex {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    if self.patch.is_empty() {
      write!(f, "{}.{}", self.major, self.minor)
    } else {
      write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Data, Hash, Dummy)]
pub enum UpdateStatus {
  Error,
  UpToDate,
  Discrepancy(VersionComplex),
  Patch(VersionComplex),
  Minor(VersionComplex),
  Major(VersionComplex),
}

impl Display for UpdateStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    match self {
      UpdateStatus::Major(remote) => write!(f, "Major update available: {remote}"),
      UpdateStatus::Minor(remote) => write!(f, "Minor update available: {remote}"),
      UpdateStatus::Patch(remote) => write!(f, "Patch available: {remote}"),
      UpdateStatus::UpToDate => write!(f, "Up to date"),
      UpdateStatus::Error => write!(f, "Error"),
      UpdateStatus::Discrepancy(_) => write!(f, "Discrepancy"),
    }
  }
}

impl<VL: Borrow<VersionComplex>, VR: Borrow<VersionComplex>> From<(VL, Option<VR>)>
  for UpdateStatus
{
  fn from((local, remote): (VL, Option<VR>)) -> Self {
    if let Some(remote) = remote {
      let local = local.borrow();
      let remote = remote.borrow().clone();

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

impl From<(&ModVersionMeta, &Option<ModVersionMeta>)> for UpdateStatus {
  fn from((local, remote): (&ModVersionMeta, &Option<ModVersionMeta>)) -> Self {
    (&local.version, remote.as_ref().map(|r| &r.version)).into()
  }
}

impl From<&UpdateStatus> for KeyOrValue<Color> {
  fn from(status: &UpdateStatus) -> Self {
    match status {
      UpdateStatus::Major(_) => ORANGE_KEY.into(),
      UpdateStatus::Minor(_) => YELLOW_KEY.into(),
      UpdateStatus::Patch(_) => BLUE_KEY.into(),
      UpdateStatus::Discrepancy(_) => Color::from_hex_str("#810181").unwrap().into(),
      UpdateStatus::Error => RED_KEY.into(),
      UpdateStatus::UpToDate => GREEN_KEY.into(),
    }
  }
}

impl UpdateStatus {
  pub fn as_text_colour(&self) -> KeyOrValue<Color> {
    match self {
      UpdateStatus::Major(_) => ON_ORANGE_KEY.into(),
      UpdateStatus::Minor(_) => ON_YELLOW_KEY.into(),
      UpdateStatus::Patch(_) => ON_BLUE_KEY.into(),
      UpdateStatus::Discrepancy(_) => Color::from_hex_str("#ffd6f7").unwrap().into(),
      UpdateStatus::Error => ON_RED_KEY.into(),
      UpdateStatus::UpToDate => ON_GREEN_KEY.into(),
    }
  }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Data, Lens, Default, Hash, Dummy)]
pub struct ModMetadata {
  #[data(same_fn = "PartialEq::eq")]
  pub install_date: Option<DateTime<Utc>>,
}

impl ModMetadata {
  const FILE_NAME: &'static str = ".moss";
  pub const SUBMIT_MOD_METADATA: Selector<(String, ModMetadata)> =
    Selector::new("mod_metadata.submit");

  pub fn new() -> Self {
    Self {
      install_date: Some(Utc::now()),
    }
  }

  pub fn path(parent: impl AsRef<Path>) -> PathBuf {
    parent.as_ref().join(Self::FILE_NAME)
  }

  pub async fn parse(mod_folder: impl AsRef<Path>) -> std::io::Result<Self> {
    use tokio::fs::read_to_string;

    let json = read_to_string(Self::path(mod_folder)).await?;

    let metadata = serde_json::from_str(&json)?;

    Ok(metadata)
  }

  pub async fn parse_and_send(
    id: String,
    mod_folder: impl AsRef<Path>,
    ext_ctx: Option<ExtEventSink>,
  ) -> Option<ModMetadata> {
    use druid::Target;

    if let Ok(mod_metadata) = Self::parse(mod_folder).await {
      if let Some(ext_ctx) = ext_ctx {
        let _ = ext_ctx.submit_command(Self::SUBMIT_MOD_METADATA, (id, mod_metadata), Target::Auto);
      } else {
        return Some(mod_metadata);
      }
    }
    None
  }

  pub async fn save(&self, mod_folder: impl AsRef<Path>) -> std::io::Result<()> {
    use tokio::fs::write;

    let path = Self::path(mod_folder);

    let json = serde_json::to_vec_pretty(&self)?;

    write(&path, json).await?;

    Ok(())
  }
}

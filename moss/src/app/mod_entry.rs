use core::fmt;
use std::{
  borrow::{Borrow, Cow},
  fmt::Display,
  fs::File,
  hash::Hash,
  io::{BufRead, BufReader},
  path::{Path, PathBuf},
  rc::Rc,
  sync::{Arc, LazyLock},
};

use ahash::AHashMap;
use chrono::{DateTime, Local, Utc};
use common::{
  controllers::{next_id, MaxSizeBox, SharedIdHoverState},
  labels::LabelExt as _,
  lenses::LensExtExt as _,
  widget_ext::{WidgetExtEx as _, WithHoverIdState},
  widgets::card::Card,
};
use druid::{
  kurbo::Line,
  lens, theme,
  widget::{Button, Checkbox, Either, Flex, Label, Painter, ViewSwitcher},
  Color, Data, ExtEventSink, KeyOrValue, Lens, LensExt, RenderContext as _, Selector, Widget,
  WidgetExt,
};
use druid_patch::table::{FlexTable, RowData};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as _};
use fake::Dummy;
use icons::{NEW_RELEASES, REPORT, SICK, THUMB_UP};
use json_comments::StripComments;
use serde::{Deserialize, Serialize};
use serde_aux::prelude::*;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub mod version_checker;

pub use version_checker::{ModVersionMeta, UpdateStatus, VersionChecker};
use web_client::WebClient;

use crate::{
  app::{
    app_delegate::AppCommands,
    controllers::GLOBAL_ASYNC_CONTROLLER,
    mod_description::{notify_enabled, ModDescription},
    mod_list::{headings::Heading, ModList, ModMap},
    util::{self, default_true, parse_game_version, Tap},
    App, SharedFromEnv,
  },
  nav_bar::{Nav, NavLabel},
  theme::{
    BLUE_KEY, GREEN_KEY, ON_BLUE_KEY, ON_GREEN_KEY, ON_ORANGE_KEY, ON_RED_KEY, ON_YELLOW_KEY,
    ORANGE_KEY, RED_KEY, YELLOW_KEY,
  },
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
  #[serde(alias = "id")]
  pub mod_id: String,
  #[serde(skip)]
  #[data(eq)]
  #[dummy(default)]
  pub internal_id: Uuid,
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
  pub dependencies: Arc<Vec<Dependency>>,
  #[serde(
    alias = "totalConversion",
    default,
    deserialize_with = "deserialize_bool_from_anything"
  )]
  pub total_conversion: bool,
  #[serde(skip)]
  pub enabled: bool,
  #[serde(skip)]
  #[dummy(default)]
  pub version_checker: Option<VersionChecker>,
  #[serde(skip)]
  #[data(eq)]
  pub path: PathBuf,
  #[serde(skip)]
  #[serde(default = "default_true")]
  display: bool,
  #[serde(skip)]
  pub manager_metadata: ModMetadata,
  #[serde(skip, default)]
  #[data(ignore)]
  pub view_state: T,

  #[serde(skip, default)]
  pub duplicates: Arc<Vec<ModEntry<T>>>,
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
    write!(f, "{}", name.as_ref().unwrap_or(id))?;
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
      v.as_ref().map(|v| &v.local.fractal_id).and_then(|s| {
        (!s.is_empty()).then(|| format!("{}{}", ModDescription::FRACTAL_URL, s.clone()))
      })
    })
  }

  pub fn nexus_link() -> impl Lens<Self, Option<String>> {
    Self::version_checker.compute(|v| {
      v.as_ref().map(|v| &v.local.nexus_id).and_then(|s| {
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

  pub fn enable_dependencies(id: &str, data: &mut App) -> bool {
    if !ViewModEntry::enable_all_dependencies(id, &mut data.mod_list.mods) {
      App::mod_list
        .then(ModList::mods)
        .index(id)
        .then(ModEntry::enabled.in_rc())
        .put(data, false);
    }

    false
  }

  pub fn enable_all_dependencies<'a>(id: &str, mods: &mut ModMap) -> bool {
    let mut mods: AHashMap<_, _> = mods.iter_mut().map(|(k, v)| (k.as_str(), v)).collect();

    let entry = mods.remove(id).unwrap();

    let mut checked = Vec::with_capacity(entry.dependencies.len());
    let mut hashes = AHashMap::with_capacity(entry.dependencies.len());

    fn get_all_dependencies<'a>(
      mods: &mut AHashMap<&str, &'a mut Rc<ViewModEntry>>,
      dep: &Dependency,
      hashes: &mut AHashMap<u64, Option<u64>>,
      checked: &mut Vec<&'a mut Rc<ViewModEntry>>,
    ) -> bool {
      let hash_id = hashes.hasher().hash_one(&dep.id);
      let hash_val = dep
        .version
        .as_ref()
        .map(|v| hashes.hasher().hash_one(v.major()));
      if let Err(err) = hashes.try_insert(hash_id, hash_val) {
        return err.entry.remove() == err.value;
      }

      if let Some(entry) = mods.remove(dep.id.as_str())
        && dep
          .version
          .as_ref()
          .is_none_or(|v| v.major() == entry.version.major())
      {
        for sub_dep in entry.dependencies.iter() {
          if !get_all_dependencies(mods, sub_dep, hashes, checked) {
            return false;
          }
        }

        checked.push(entry);

        true
      } else {
        false
      }
    }

    for dep in entry.dependencies.iter() {
      if !get_all_dependencies(&mut mods, dep, &mut hashes, &mut checked) {
        return false;
      }
    }

    for dep in checked {
      ModEntry::enabled.in_rc().put(dep, true);
    }

    ModEntry::enabled.in_rc().put(entry, true);

    true
  }

  pub fn spawn_version_check(&self, client: &Arc<WebClient>) {
    if let Some(version_checker) = self.version_checker.as_ref() {
      let mod_id = self.mod_id.clone();
      let internal_id = self.internal_id;
      GLOBAL_ASYNC_CONTROLLER.get_unchecked().add_task_with_check(
        util::get_master_version(&client, version_checker.local.remote_url.clone()),
        {
          let mod_id = mod_id.clone();
          move |_, _, app, _| {
            app
              .mod_list
              .mods
              .get(&mod_id)
              .into_iter()
              .flat_map(|entry| entry.iter_with_dupes())
              .any(|entry| entry.internal_id == internal_id)
          }
        },
        move |res, _, app, _| {
          if let Some(entry) = app.mod_list.mods.get_mut(&mod_id)
            && let entry = Rc::make_mut(entry)
            && entry.internal_id == internal_id
            && let Some(version_checker) = entry.version_checker.as_mut()
          {
            version_checker.update_remote(res.ok());
          }
        },
      );
    }
  }

  pub fn get_direct_download_url(&self) -> Option<&str> {
    self
      .version_checker
      .as_ref()
      .and_then(|vc| vc.get_direct_download_url())
  }

  pub fn iter_with_dupes(&self) -> impl Iterator<Item = &Self> {
    std::iter::once(self).chain(self.duplicates.iter())
  }

  pub fn iter_with_dupes_mut(&mut self, mut func: impl FnMut(&mut Self))
  where
    T: Clone,
  {
    (func)(self);
    let duplicates = Arc::make_mut(&mut self.duplicates);
    for dupe in duplicates {
      (func)(dupe)
    }
  }
}

impl<T: Default> ModEntry<T> {
  pub fn from_file(path: &Path, manager_metadata: ModMetadata) -> Result<Self, ModEntryError> {
    let mod_info_file = std::fs::read_to_string(path.join("mod_info.json"))?;
    let stripped = std::io::read_to_string(StripComments::new(mod_info_file.as_bytes()))?;
    let mut mod_info = json5::from_str::<Self>(&stripped)?;
    mod_info.version_checker = ModEntry::parse_version_checker(path, &mod_info.mod_id);
    mod_info.path = path.to_path_buf();
    mod_info.manager_metadata = manager_metadata;
    Ok(mod_info)
  }
}

impl ModEntry {
  pub const ASK_DELETE_MOD: Selector<ModEntry> = Selector::new("mod_entry.delete");
  pub const AUTO_UPDATE: Selector<ModEntry> = Selector::new("mod_list.update.auto");
  pub const REPLACE: Selector<ModEntry> = Selector::new("MOD_ENTRY_REPLACE");
  pub const VERSION_CHECK_COMPLETE: Selector = Selector::new("mod_entry.version_check.complete");

  fn parse_version_checker(path: &Path, id: &str) -> Option<VersionChecker> {
    static VC_LOCATION_PATH: LazyLock<&'static Path> =
      LazyLock::new(|| Path::new("data/config/version/version_files.csv"));

    if let Ok(version_loc_file) = File::open(path.join(*VC_LOCATION_PATH))
      && let Some(Ok(version_filename)) = BufReader::new(version_loc_file).lines().nth(1)
      && let Some(version_filename) = version_filename.split(',').next()
      && let Ok(version_data) = std::fs::read_to_string(path.join(version_filename))
      && let Ok(no_comments) = std::io::read_to_string(StripComments::new(version_data.as_bytes()))
      && let Ok(normalized) = handwritten_json::normalize(&no_comments)
      && let Ok(mut version) = json5::from_str::<ModVersionMeta>(&normalized)
    {
      version.id = id.to_string();
      Some(VersionChecker::new(version))
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

  fn deserialize_dependencies<'de, D>(deserializer: D) -> Result<Arc<Vec<Dependency>>, D::Error>
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

    Ok(Arc::new(
      dependencies
        .into_iter()
        .filter_map(|RawDependency { id, name, version }| {
          id.map(|id| Dependency { id, name, version })
        })
        .collect(),
    ))
  }
}

impl installer::Entry for ModEntry {
  type Id = String;
  type ParseError = ModEntryError;
  type EnrichmentError = ModEntryError;
  type Context = Arc<WebClient>;

  fn id(&self) -> Self::Id {
    self.mod_id.clone()
  }

  fn destination_folder(&self, parent: &Path) -> PathBuf {
    parent.join("mods").join(self.id())
  }

  async fn parse(path: impl AsRef<Path>) -> Result<Self, ModEntryError> {
    let path = path.as_ref();
    let metadata = ModMetadata::default();
    metadata.save(path).await?;
    ModEntry::from_file(path, metadata)
  }

  async fn enrich(&mut self, context: &Self::Context, path: PathBuf) -> Result<(), ModEntryError> {
    self.manager_metadata.save(&path).await?;

    self.set_path(path);
    self.spawn_version_check(context);

    Ok(())
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
            Heading::ID => label.lens(ViewModEntry::mod_id).padding(5.).expand_width(),
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
          |entry: &ViewModEntry, _| entry.get_direct_download_url().is_some(),
          Either::new(
            |entry: &ViewModEntry, _| {
              entry
                .version_checker
                .as_ref()
                .is_some_and(|vc| vc.update_status != UpdateStatus::Error)
            },
            Either::new(
              |entry: &ViewModEntry, _| {
                entry.version_checker.as_ref().is_some_and(|vc| {
                  !matches!(
                    vc.update_status,
                    UpdateStatus::UpToDate | UpdateStatus::Discrepancy(_)
                  )
                })
              },
              Button::from_label(Label::wrapped("Update available!")),
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

                let tooltip = match &update_status {
                  UpdateStatus::Error => "Error\nThere was an error retrieving or parsing this \
                                          mod's version information."
                    .to_string(),
                  UpdateStatus::Discrepancy(remote) => {
                    format!(
                      "Discrepancy\nThe installed version of this mod is newer than the known \
                       latest version.\nNewest version according to server: {remote}.\nThis is \
                       usually because the author has forgotten to update their version file and \
                       is not an error."
                    )
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
                        330.0,
                      )),
                    )
                    .with_offset((10.0, 10.0)),
                );
              })
              .boxed()
          } else {
            Label::dynamic(|data: &(Option<UpdateStatus>, Version), _| data.1.to_string()).boxed()
          }
        },
      ),
      Label::dynamic(|data: &(Option<UpdateStatus>, Version), _| data.1.to_string()),
    )
    .lens((
      ViewModEntry::version_checker.compute(|vc| vc.as_ref().map(|vc| vc.update_status.clone())),
      ViewModEntry::version,
    ))
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
    self.mod_id.hash(state);
    self.name.hash(state);
    self.author.hash(state);
    self.version.hash(state);
    self.description.hash(state);
    self.game_version.hash(state);
    self.enabled.hash(state);
    self.version_checker.hash(state);
    self.path.hash(state);
    self.manager_metadata.hash(state);
  }
}

impl<T> PartialEq for ModEntry<T> {
  fn eq(&self, other: &Self) -> bool {
    self.mod_id == other.mod_id
      && self.name == other.name
      && self.author == other.author
      && self.version == other.version
      && self.description == other.description
      && self.game_version == other.game_version
      && self.enabled == other.enabled
      && self.version_checker == other.version_checker
      && self.path == other.path
      && self.display == other.display
      && self.manager_metadata == other.manager_metadata
  }
}

impl<T> Eq for ModEntry<T> {}

impl From<ModEntry> for ViewModEntry {
  fn from(
    ModEntry {
      mod_id,
      internal_id,
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
      path,
      display,
      manager_metadata,
      view_state: (),
      duplicates,
    }: ModEntry,
  ) -> Self {
    ViewModEntry {
      mod_id,
      internal_id,
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
      path,
      display,
      manager_metadata,
      view_state: ViewState::new(),
      duplicates: Arc::new(
        Arc::unwrap_or_clone(duplicates)
          .into_iter()
          .map(|dup| dup.into())
          .collect(),
      ),
    }
  }
}

impl From<ViewModEntry> for ModEntry {
  fn from(
    ViewModEntry {
      mod_id,
      internal_id,
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
      path,
      display,
      manager_metadata,
      view_state: _,
      duplicates,
    }: ViewModEntry,
  ) -> Self {
    ModEntry {
      mod_id,
      internal_id,
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
      path,
      display,
      manager_metadata,
      view_state: (),
      duplicates: Arc::new(
        Arc::unwrap_or_clone(duplicates)
          .into_iter()
          .map(|dup| dup.into())
          .collect(),
      ),
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
    self.mod_id.clone()
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

#[derive(Debug, thiserror::Error)]
pub enum ModEntryError {
  #[error("JSON5 parsing error")]
  JsonError(#[from] json5::Error),
  #[error("I/O error")]
  IoError(#[from] std::io::Error),
  #[error("VC error: {0}")]
  VersionCheckError(Arc<anyhow::Error>),
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
    let path = Self::path(mod_folder);

    let json = serde_json::to_vec_pretty(&self)?;

    let mut file = tokio::fs::File::create(path).await?;

    file.write_all(&json).await?;
    file.sync_all().await
  }

  pub fn save_blocking(&self, mod_folder: impl AsRef<Path>) -> std::io::Result<()> {
    tokio::runtime::Handle::current().block_on(self.save(mod_folder))
  }
}

#[cfg(test)]
mod test {
  use crate::app::{
    mod_entry::{Dependency, Version, VersionComplex, ViewModEntry},
    mod_list::ModMap,
  };

  #[test]
  fn enable_dependencies_minimal() {
    // Setup
    let dep = Dependency {
      id: "Dep".to_owned(),
      name: None,
      version: Some(Version::Complex(VersionComplex {
        major: 1,
        minor: 0,
        patch: 0.to_string(),
      })),
    };
    let dep_entry = ViewModEntry {
      mod_id: "Dep".to_owned(),
      version: Version::Complex(VersionComplex {
        major: 1,
        minor: 0,
        patch: 0.to_string(),
      }),
      ..Default::default()
    };
    let entry = ViewModEntry {
      mod_id: "Entry".to_owned(),
      dependencies: vec![dep].into(),
      ..Default::default()
    };

    let mut mods = ModMap::new();
    mods.extend([
      ("Dep".to_owned(), dep_entry.into()),
      ("Entry".to_owned(), entry.into()),
    ]);

    // Assert
    assert!(!mods["Entry"].enabled);
    assert!(!mods["Dep"].enabled);

    assert!(ViewModEntry::enable_all_dependencies("Entry", &mut mods));

    assert!(mods["Entry"].enabled);
    assert!(mods["Dep"].enabled);
  }

  #[test]
  fn enable_dependencies() {
    // Setup
    let sub_dep_entry = ViewModEntry {
      mod_id: "subdep".to_owned(),
      version: Version::Complex(VersionComplex {
        major: 0,
        minor: 2,
        patch: "some-RC99".to_owned(),
      }),
      ..Default::default()
    };
    let dep_entry = ViewModEntry {
      mod_id: "Dep".to_owned(),
      version: Version::Complex(VersionComplex {
        major: 1,
        minor: 0,
        patch: 0.to_string(),
      }),
      dependencies: vec![Dependency {
        id: "subdep".to_owned(),
        name: None,
        version: Some(Version::Complex(VersionComplex {
          major: 0,
          minor: 5,
          patch: "foo".to_owned(),
        })),
      }]
      .into(),
      ..Default::default()
    };
    let entry = ViewModEntry {
      mod_id: "Entry".to_owned(),
      dependencies: vec![Dependency {
        id: "Dep".to_owned(),
        name: None,
        version: Some(Version::Complex(VersionComplex {
          major: 1,
          minor: 0,
          patch: 0.to_string(),
        })),
      }]
      .into(),
      ..Default::default()
    };

    let unused_entry_a = ViewModEntry {
      mod_id: "Unused A".to_owned(),
      ..Default::default()
    };
    let unused_entry_b = ViewModEntry {
      mod_id: "Unused B".to_owned(),
      ..Default::default()
    };

    let mut mods = ModMap::new();
    mods.extend([
      ("Dep".to_owned(), dep_entry.into()),
      ("Entry".to_owned(), entry.into()),
      ("subdep".to_owned(), sub_dep_entry.into()),
      ("Unused A".to_owned(), unused_entry_a.into()),
      ("Unused B".to_owned(), unused_entry_b.into()),
    ]);

    // Assert
    assert!(mods.values().all(|entry| !entry.enabled));

    assert!(ViewModEntry::enable_all_dependencies("Entry", &mut mods));

    assert!(mods["Entry"].enabled);
    assert!(mods["Dep"].enabled);
    assert!(mods["subdep"].enabled);
    assert!(!mods["Unused A"].enabled);
    assert!(!mods["Unused B"].enabled);
  }

  #[test]
  fn missing_dependency() {
    let entry = ViewModEntry {
      mod_id: "entry".to_owned(),
      dependencies: vec![Dependency {
        id: "doesn't exist".to_owned(),
        name: None,
        version: None,
      }]
      .into(),
      ..Default::default()
    };

    let mut mods = ModMap::new();
    mods.extend([("entry".to_owned(), entry.into())]);

    assert!(!ViewModEntry::enable_all_dependencies("entry", &mut mods));
    assert!(!mods["entry"].enabled);
  }
}

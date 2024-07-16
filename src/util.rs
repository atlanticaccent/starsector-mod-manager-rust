use std::{
  any::Any,
  borrow::Borrow,
  collections::{HashMap, VecDeque},
  fmt::Debug,
  hash::Hash,
  io::Read,
  marker::PhantomData,
  ops::{Deref, DerefMut, Index, IndexMut},
  path::PathBuf,
  rc::Rc,
  sync::{Arc, Mutex, Weak},
};

use druid::{
  keyboard_types, lens,
  lens::{Constant, Then},
  text::{Attribute, AttributeSpans, RichText},
  theme,
  widget::{
    Align, Axis, Controller, ControllerHost, DefaultScopePolicy, Either, Flex, Label, LabelText,
    LensScopeTransfer, LensWrap, Painter, RawLabel, Scope, ScopeTransfer, SizedBox,
  },
  Color, Command, Data, Env, Event, EventCtx, ExtEventSink, FontWeight, Key, KeyOrValue, Lens,
  LensExt as _, MouseEvent, Point, RenderContext, Selector, Target, TimerToken, UnitPoint, Widget,
  WidgetExt, WidgetId,
};
use druid_widget_nursery::{
  animation::Interpolate,
  prism::{Closures, Prism, PrismWrap},
  stack_tooltip::StackTooltip,
  CommandCtx, Mask,
};
use json_comments::strip_comments;
use lazy_static::lazy_static;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use tokio::{select, sync::mpsc};

use crate::{
  app::{
    controllers::{ExtensibleController, HoverController, OnEvent, OnNotif},
    mod_entry::{GameVersion, ModEntry, ModVersionMeta},
  },
  patch::click::Click,
  widgets::card::{Card, CardBuilder},
};

pub(crate) mod icons;

pub use icons::*;

use super::{
  controllers::{
    next_id, ConstraintId, DelayedPainter, HeightLinkerShared, HoverState, InvisibleIf,
    LayoutRepeater, LinkedHeights, OnHover, SharedConstraint, SharedIdHoverState,
  },
  overlays::Popup,
};

pub const ORANGE_KEY: Key<Color> = Key::new("util.colour.orange");
pub const BLUE_KEY: Key<Color> = Key::new("util.colour.blue");
pub const GREEN_KEY: Key<Color> = Key::new("util.colour.green");
pub const RED_KEY: Key<Color> = Key::new("util.colour.red");
pub const YELLOW_KEY: Key<Color> = Key::new("util.colour.yellow");
pub const ON_GREEN_KEY: Key<Color> = Key::new("util.colour.on_green");
pub const ON_RED_KEY: Key<Color> = Key::new("util.colour.on_red");
pub const ON_YELLOW_KEY: Key<Color> = Key::new("util.colour.on_yellow");
pub const ON_BLUE_KEY: Key<Color> = Key::new("util.colour.on_blue");
pub const ON_ORANGE_KEY: Key<Color> = Key::new("util.colour.on_orange");

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
  #[error("No such file")]
  NoSuchFile,
  #[error("File read error")]
  ReadError,
  #[error("File format error")]
  FormatError,
  #[error("Archive error")]
  ZipError(#[from] zip::result::ZipError),
  #[error("IO error")]
  IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub enum SaveError {
  File,
  Write,
  Format,
}

pub fn get_quoted_version(
  starsector_version: &(
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
  ),
) -> Option<String> {
  match starsector_version {
    (None, None, None, None) => None,
    (major, minor, patch, rc) => Some(format!(
      "{}.{}{}{}",
      major.clone().unwrap_or_else(|| "0".to_string()),
      minor.clone().unwrap_or_default(),
      patch
        .clone()
        .map_or_else(|| "".to_string(), |p| format!(".{}", p)),
      rc.clone()
        .map_or_else(|| "".to_string(), |rc| format!("a-RC{}", rc))
    )),
  }
}

pub trait LabelExt<T: Data> {
  fn wrapped(label: impl AsRef<str>) -> Label<T> {
    Label::new(label.as_ref()).with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }

  fn wrapped_lens<U: Data, L: Lens<T, U>>(lens: L) -> LensWrap<T, String, L, Label<String>> {
    LensWrap::new(
      Label::dynamic(|t: &String, _| t.to_string())
        .with_line_break_mode(druid::widget::LineBreaking::WordWrap),
      lens,
    )
  }

  fn wrapped_func<F, S>(func: F) -> Label<T>
  where
    S: Into<Arc<str>>,
    F: Fn(&T, &druid::Env) -> S + 'static,
  {
    Label::new(func).with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }

  fn wrapped_into(label: impl Into<LabelText<T>>) -> Label<T> {
    Label::new(label).with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }

  fn stringify() -> Label<T>
  where
    T: ToString,
  {
    Label::new(|t: &T, _: &Env| t.to_string())
  }

  fn stringify_wrapped() -> Label<T>
  where
    T: ToString,
  {
    Label::stringify().with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }
}

impl<T: Data> LabelExt<T> for Label<T> {}

pub fn make_flex_pair<T: Data>(
  label: impl Widget<T> + 'static,
  ratio_1: f64,
  val: impl Widget<T> + 'static,
  ratio_2: f64,
  axis: Axis,
) -> Flex<T> {
  Flex::for_axis(axis)
    .with_flex_child(label.expand_width(), ratio_1)
    .with_flex_child(val.expand_width(), ratio_2)
}

pub fn make_flex_description_row<T: Data>(
  label: impl Widget<T> + 'static,
  val: impl Widget<T> + 'static,
) -> Flex<T> {
  make_flex_pair(label, 1., val, 1.5, Axis::Horizontal)
}

pub fn make_flex_settings_row<T: Data>(
  widget: impl Widget<T> + 'static,
  label: impl Widget<T> + 'static,
) -> Flex<T> {
  make_flex_pair(
    widget.align_horizontal(UnitPoint::CENTER),
    1.,
    label,
    10.,
    Axis::Horizontal,
  )
}

pub fn make_flex_column_pair<T: Data>(
  label: impl Widget<T> + 'static,
  val: impl Widget<T> + 'static,
) -> Flex<T> {
  make_flex_pair(label, 1., val, 1., Axis::Vertical)
}

pub fn make_pair<T: Data>(
  label: impl Widget<T> + 'static,
  val: impl Widget<T> + 'static,
  axis: Axis,
) -> Flex<T> {
  Flex::for_axis(axis)
    .with_child(label.expand_width())
    .with_child(val.expand_width())
}

pub fn make_description_row<T: Data>(
  label: impl Widget<T> + 'static,
  val: impl Widget<T> + 'static,
) -> Flex<T> {
  make_pair(label, val, Axis::Horizontal)
}

pub fn make_column_pair<T: Data>(
  label: impl Widget<T> + 'static,
  val: impl Widget<T> + 'static,
) -> Flex<T> {
  make_pair(label, val, Axis::Vertical)
}

pub const MASTER_VERSION_RECEIVED: Selector<(String, Result<ModVersionMeta, String>)> =
  Selector::new("remote_version_received");

pub async fn get_master_version(
  client: &Client,
  ext_sink: Option<ExtEventSink>,
  local: &ModVersionMeta,
) -> Option<ModVersionMeta> {
  let res = send_request(client, local.remote_url.clone()).await;

  let payload = match res {
    Err(err) => (local.id.clone(), Err(err)),
    Ok(remote) => {
      let mut stripped = String::new();
      if strip_comments(remote.as_bytes())
        .read_to_string(&mut stripped)
        .is_ok()
        && let Ok(normalized) = handwritten_json::normalize(&stripped)
        && let Ok(remote) = json5::from_str::<ModVersionMeta>(&normalized)
      {
        (local.id.clone(), Ok(remote))
      } else {
        (
          local.id.clone(),
          Err(format!("Parse error. Payload:\n{}", remote)),
        )
      }
    }
  };

  if let Some(ext_sink) = ext_sink {
    if let Err(err) = ext_sink.submit_command(MASTER_VERSION_RECEIVED, payload, Target::Auto) {
      eprintln!("Failed to submit remote version data {}", err);
    }
    None
  } else {
    payload.1.ok()
  }
}

async fn send_request(client: &Client, url: String) -> Result<String, String> {
  let request = client.get(url).build().map_err(|e| format!("{:?}", e))?;

  client
    .execute(request)
    .await
    .map_err(|e| format!("{:?}", e))?
    .error_for_status()
    .map_err(|e| format!("{:?}", e))?
    .text()
    .await
    .map_err(|e| format!("{:?}", e))
}

pub fn bold_text<T: Data>(
  text: &str,
  size: impl Into<KeyOrValue<f64>>,
  weight: FontWeight,
  colour: impl Into<KeyOrValue<Color>>,
) -> impl Widget<T> {
  RawLabel::new()
    .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
    .lens(lens::Constant(RichText::new_with_attributes(
      text.into(),
      AttributeSpans::new().tap(|s| {
        s.add(0..text.len(), Attribute::Weight(weight));
        s.add(0..text.len(), Attribute::FontSize(size.into()));
        s.add(0..text.len(), Attribute::TextColor(colour.into()));
      }),
    )))
}

pub fn h1_fixed<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(text, 24., FontWeight::BOLD, theme::TEXT_COLOR)
}

pub fn h2_fixed<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(text, 20., FontWeight::SEMI_BOLD, theme::TEXT_COLOR)
}

pub fn h3_fixed<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(text, 18., FontWeight::MEDIUM, theme::TEXT_COLOR)
}

pub fn bolded<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(
    text,
    theme::TEXT_SIZE_NORMAL,
    FontWeight::MEDIUM,
    theme::TEXT_COLOR,
  )
}

pub fn lensed_bold<T: Data + AsRef<str>>(
  size: impl Into<KeyOrValue<f64>>,
  weight: FontWeight,
  colour: impl Into<KeyOrValue<Color>>,
) -> impl Widget<T> {
  let size = size.into();
  let colour = colour.into();
  RawLabel::new()
    .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
    .lens(Compute::new(move |data: &T| {
      let text = data.as_ref();
      let mut attributes = AttributeSpans::new();
      attributes.add(0..text.len(), Attribute::Weight(weight));
      attributes.add(0..text.len(), Attribute::FontSize(size.clone()));
      attributes.add(0..text.len(), Attribute::TextColor(colour.clone()));
      RichText::new_with_attributes(text.into(), attributes)
    }))
}

pub fn h1<T: Data + AsRef<str>>() -> impl Widget<T> {
  lensed_bold(24., FontWeight::BOLD, theme::TEXT_COLOR)
}

pub fn h2<T: Data + AsRef<str>>() -> impl Widget<T> {
  lensed_bold(20., FontWeight::SEMI_BOLD, theme::TEXT_COLOR)
}

pub fn h3<T: Data + AsRef<str>>() -> impl Widget<T> {
  lensed_bold(18., FontWeight::MEDIUM, theme::TEXT_COLOR)
}

pub const GET_INSTALLED_STARSECTOR: Selector<Result<GameVersion, LoadError>> =
  Selector::new("util.starsector_version.get");

pub async fn get_starsector_version(ext_ctx: ExtEventSink, install_dir: PathBuf) {
  use classfile_parser::class_parser;
  use regex::bytes::Regex;
  use tokio::{fs, task};

  #[cfg(target_os = "linux")]
  let obf_jar = install_dir.join("starfarer_obf.jar");
  #[cfg(target_os = "windows")]
  let obf_jar = install_dir.join("starsector-core/starfarer_obf.jar");
  #[cfg(target_os = "macos")]
  let obf_jar = install_dir.join("Contents/Resources/Java/starfarer_obf.jar");

  let mut res = task::spawn_blocking(move || {
    let file = std::fs::File::open(obf_jar)?;
    let mut zip = zip::ZipArchive::new(file)?;

    // println!("{:?}", zip.file_names().collect::<Vec<&str>>());

    let mut version_class = zip
      .by_name("com/fs/starfarer/Version.class")
      .map_err(|_| LoadError::NoSuchFile)?;

    let mut buf: Vec<u8> = Vec::new();
    version_class
      .read_to_end(&mut buf)
      .map_err(|_| LoadError::ReadError)
      .and_then(|_| {
        class_parser(&buf)
          .map_err(|_| LoadError::FormatError)
          .map(|(_, class_file)| class_file)
      })
      .and_then(|class_file| {
        class_file
          .fields
          .iter()
          .find_map(|f| {
            if let classfile_parser::constant_info::ConstantInfo::Utf8(name) =
              &class_file.const_pool[(f.name_index - 1) as usize]
              && name.utf8_string == "versionOnly"
              && let Ok((_, attr)) =
                classfile_parser::attribute_info::constant_value_attribute_parser(
                  &f.attributes.first().unwrap().info,
                )
              && let classfile_parser::constant_info::ConstantInfo::Utf8(utf_const) =
                &class_file.const_pool[attr.constant_value_index as usize]
            {
              Some(utf_const.utf8_string.clone())
            } else {
              None
            }
          })
          .ok_or(LoadError::FormatError)
      })
  })
  .await
  .map_err(|_| LoadError::ReadError)
  .flatten();

  if res.is_err() {
    lazy_static! {
      static ref RE: Regex = Regex::new(r"Starting Starsector (.*) launcher").unwrap();
    }
    res = fs::read(install_dir.join("starsector-core").join("starsector.log"))
      .await
      .map_err(|_| LoadError::ReadError)
      .and_then(|file| {
        RE.captures(&file)
          .and_then(|captures| captures.get(1))
          .ok_or(LoadError::FormatError)
          .and_then(|m| {
            String::from_utf8(m.as_bytes().to_vec()).map_err(|_| LoadError::FormatError)
          })
      })
  };

  let parsed = res.map(|text| parse_game_version(&text));

  if ext_ctx
    .submit_command(GET_INSTALLED_STARSECTOR, parsed, Target::Auto)
    .is_err()
  {
    eprintln!("Failed to submit starsector version back to main thread")
  };
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
pub fn parse_game_version(
  text: &str,
) -> (
  Option<String>,
  Option<String>,
  Option<String>,
  Option<String>,
) {
  lazy_static! {
    static ref VERSION_REGEX: Regex = Regex::new(r"(?i)\.|a-rc|a").unwrap();
  }
  let components: Vec<&str> = VERSION_REGEX
    .split(text)
    .filter(|c| !c.is_empty())
    .collect();

  match components.as_slice() {
    [major, minor] if major == &"0" => {
      // text = format!("{}.{}a", major, minor);
      (Some(major.to_string()), Some(minor.to_string()), None, None)
    }
    [minor, patch_rc] => {
      // text = format!("0.{}a-RC{}", minor, rc);
      if text.contains("a-RC") {
        (
          Some("0".to_string()),
          Some(minor.to_string()),
          None,
          Some(patch_rc.to_string()),
        )
      } else {
        (
          Some("0".to_string()),
          Some(minor.to_string()),
          Some(patch_rc.to_string()),
          None,
        )
      }
    }
    [major, minor, patch_rc] if major == &"0" => {
      // text = format!("{}.{}a-RC{}", major, minor, rc);
      if text.contains("a-RC") {
        (
          Some(major.to_string()),
          Some(minor.to_string()),
          None,
          Some(patch_rc.to_string()),
        )
      } else {
        (
          Some(major.to_string()),
          Some(minor.to_string()),
          Some(patch_rc.to_string()),
          None,
        )
      }
    }
    [minor, patch, rc] => {
      // text = format!("0.{}.{}a-RC{}", minor, patch, rc);
      (
        Some("0".to_string()),
        Some(minor.to_string()),
        Some(patch.to_string()),
        Some(rc.to_string()),
      )
    }
    [major, minor, patch, rc] if major == &"0" => {
      // text = format!("{}.{}.{}a-RC{}", major, minor, patch, rc);
      (
        Some(major.to_string()),
        Some(minor.to_string()),
        Some(patch.to_string()),
        Some(rc.to_string()),
      )
    }
    _ => {
      dbg!("Failed to normalise mod's quoted game version");
      (None, None, None, None)
    }
  }
}

pub enum StarsectorVersionDiff {
  Major,
  Minor,
  Patch,
  RC,
  None,
}

impl From<(&GameVersion, &GameVersion)> for StarsectorVersionDiff {
  fn from(vals: (&GameVersion, &GameVersion)) -> Self {
    match vals {
      ((mod_major, ..), (game_major, ..)) if mod_major != game_major => {
        StarsectorVersionDiff::Major
      }
      ((_, mod_minor, ..), (_, game_minor, ..)) if mod_minor != game_minor => {
        StarsectorVersionDiff::Minor
      }
      ((.., mod_patch, _), (.., game_patch, _)) if mod_patch != game_patch => {
        StarsectorVersionDiff::Patch
      }
      ((.., mod_rc), (.., game_rc)) if mod_rc != game_rc => StarsectorVersionDiff::RC,
      _ => StarsectorVersionDiff::None,
    }
  }
}

impl From<StarsectorVersionDiff> for KeyOrValue<Color> {
  fn from(status: StarsectorVersionDiff) -> Self {
    match status {
      StarsectorVersionDiff::Major => RED_KEY.into(),
      StarsectorVersionDiff::Minor => ORANGE_KEY.into(),
      StarsectorVersionDiff::Patch => YELLOW_KEY.into(),
      StarsectorVersionDiff::RC => BLUE_KEY.into(),
      StarsectorVersionDiff::None => GREEN_KEY.into(),
    }
  }
}

#[derive(Default)]
pub struct DragWindowController {
  init_pos: Option<Point>,
  //dragging: bool
}

impl<T, W: Widget<T>> Controller<T, W> for DragWindowController {
  fn event(
    &mut self,
    child: &mut W,
    ctx: &mut EventCtx,
    event: &Event,
    data: &mut T,
    env: &druid::Env,
  ) {
    match event {
      Event::MouseDown(me) if me.buttons.has_left() => {
        ctx.set_active(true);
        self.init_pos = Some(me.window_pos)
      }
      Event::MouseMove(me) if ctx.is_active() && me.buttons.has_left() => {
        if let Some(init_pos) = self.init_pos {
          let within_window_change = me.window_pos.to_vec2() - init_pos.to_vec2();
          let old_pos = ctx.window().get_position();
          let new_pos = old_pos + within_window_change;
          ctx.window().set_position(new_pos)
        }
      }
      Event::MouseUp(_me) if ctx.is_active() => {
        self.init_pos = None;
        ctx.set_active(false)
      }
      _ => (),
    }
    child.event(ctx, event, data, env)
  }
}

#[derive(Deserialize, Clone)]
pub struct Release {
  pub name: String,
  pub tag_name: String,
  pub assets: Vec<Asset>,
}

#[derive(Deserialize, Clone)]
pub struct Asset {
  pub name: String,
  pub browser_download_url: String,
}

pub async fn get_latest_manager() -> Result<Release, String> {
  let client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_millis(500))
    .connect_timeout(std::time::Duration::from_millis(500))
    .user_agent("StarsectorModManager")
    .build()
    .map_err(|e| e.to_string())?;

  let mut res = client
    .get("https://api.github.com/repos/atlanticaccent/starsector-mod-manager-rust/releases")
    .send()
    .await
    .map_err(|e| e.to_string())?
    .json::<VecDeque<Release>>()
    .await
    .map_err(|e| e.to_string())?;

  if let Some(release) = res.pop_front() {
    Ok(release)
  } else {
    Err(String::from("Could not find any releases."))
  }
}

pub fn default_true() -> bool {
  true
}

#[derive(Clone, Data, Lens)]
pub struct IndyToggleState {
  state: bool,
}

impl ScopeTransfer for IndyToggleState {
  type In = bool;
  type State = bool;

  fn read_input(&self, _: &mut Self::State, _: &Self::In) {}

  fn write_back_input(&self, _: &Self::State, _: &mut Self::In) {}
}

impl Default for IndyToggleState {
  fn default() -> Self {
    Self { state: true }
  }
}

pub fn button_painter<T: Data>() -> Painter<T> {
  Painter::new(|ctx, _, env| {
    let is_active = ctx.is_active() && !ctx.is_disabled();
    let is_hot = ctx.is_hot();
    let size = ctx.size();
    let stroke_width = env.get(theme::BUTTON_BORDER_WIDTH);

    let rounded_rect = size
      .to_rect()
      .inset(-stroke_width / 2.0)
      .to_rounded_rect(env.get(theme::BUTTON_BORDER_RADIUS));

    let bg_gradient = if ctx.is_disabled() {
      env.get(theme::DISABLED_BUTTON_DARK)
    } else if is_active {
      env.get(theme::BUTTON_DARK)
    } else {
      env.get(theme::BUTTON_LIGHT)
    };

    let border_color = if is_hot && !ctx.is_disabled() {
      env.get(theme::BORDER_LIGHT)
    } else {
      env.get(theme::BORDER_DARK)
    };

    ctx.stroke(rounded_rect, &border_color, stroke_width);

    ctx.fill(rounded_rect, &bg_gradient);
  })
}

pub trait CommandExt: CommandCtx {
  fn submit_command_global(&mut self, cmd: impl Into<Command>) {
    let cmd: Command = cmd.into();
    self.submit_command(cmd.to(Target::Global))
  }

  fn display_popup(&mut self, popup: Popup) {
    self.submit_command(Popup::OPEN_POPUP.with(popup))
  }

  fn queue_popup(&mut self, popup: Popup) {
    self.submit_command(Popup::QUEUE_POPUP.with(popup))
  }
}

impl<T: CommandCtx> CommandExt for T {}

pub struct DummyTransfer<X, Y> {
  phantom_x: PhantomData<X>,
  phantom_y: PhantomData<Y>,
}

impl<X: Data, Y: Data> ScopeTransfer for DummyTransfer<X, Y> {
  type In = X;
  type State = Y;

  fn read_input(&self, _: &mut Self::State, _: &Self::In) {}

  fn write_back_input(&self, _: &Self::State, _: &mut Self::In) {}
}

impl<X, Y> Default for DummyTransfer<X, Y> {
  fn default() -> Self {
    Self {
      phantom_x: PhantomData,
      phantom_y: PhantomData,
    }
  }
}

pub fn hoverable_text(colour: Option<Color>) -> impl Widget<String> {
  hoverable_text_opts(colour, |w| w)
}

pub fn hoverable_text_opts<W: Widget<RichText> + 'static>(
  colour: Option<Color>,
  mut modify: impl FnMut(RawLabel<RichText>) -> W,
) -> impl Widget<String> {
  struct TextHoverController;

  impl<D: Data, W: Widget<(D, bool)>> Controller<(D, bool), W> for TextHoverController {
    fn event(
      &mut self,
      child: &mut W,
      ctx: &mut EventCtx,
      event: &Event,
      data: &mut (D, bool),
      env: &druid::Env,
    ) {
      if let Event::MouseMove(_) = event {
        data.1 = ctx.is_hot() && !ctx.is_disabled()
      }

      child.event(ctx, event, data, env)
    }
  }

  let label: RawLabel<RichText> =
    RawLabel::new().with_line_break_mode(druid::widget::LineBreaking::WordWrap);

  let wrapped = modify(label);

  Scope::from_function(
    |input: String| (input, false),
    DummyTransfer::default(),
    wrapped
      .lens(Compute::new(move |(text, hovered): &(String, bool)| {
        RichText::new(text.clone().into())
          .with_attribute(0..text.len(), Attribute::Underline(*hovered))
          .with_attribute(
            0..text.len(),
            Attribute::TextColor(
              colour
                .map(|c| c.into())
                .unwrap_or_else(|| theme::TEXT_COLOR.into()),
            ),
          )
      }))
      .controller(TextHoverController),
  )
}

pub trait WidgetExtEx<T: Data, W: Widget<T>>: Widget<T> + Sized + 'static {
  fn on_notification<CT: 'static>(
    self,
    selector: Selector<CT>,
    handler: impl Fn(&mut EventCtx, &CT, &mut T) + 'static,
  ) -> ControllerHost<Self, OnNotif<CT, T>> {
    self.controller(OnNotif::new(selector, handler))
  }

  fn on_click2(
    self,
    f: impl Fn(&mut EventCtx, &MouseEvent, &mut T, &Env) + 'static,
  ) -> ControllerHost<Self, Click<T>> {
    ControllerHost::new(self, Click::new(f))
  }

  /**
   * Sets the event as handled if the callback returns true
   */
  fn on_event(
    self,
    f: impl Fn(&mut W, &mut EventCtx, &Event, &mut T) -> bool + 'static,
  ) -> ControllerHost<Self, OnEvent<T, W>> {
    ControllerHost::new(self, OnEvent::new(f))
  }

  /**
   * Displays alternative when closure returns false
   */
  fn or_empty(self, f: impl Fn(&T, &Env) -> bool + 'static) -> Either<T> {
    Either::new(f, self, SizedBox::empty())
  }

  fn else_if(
    self,
    f: impl Fn(&T, &Env) -> bool + 'static,
    other: impl Widget<T> + 'static,
  ) -> Either<T> {
    Either::new(f, other, self)
  }

  /**
  Execute closure when command is received, with mutable access to the child
  widget.
  * Must return bool indicating if the event should be propgated to the
  child - true to propagate, false to not.
  */
  fn on_command2<CT: 'static>(
    self,
    selector: Selector<CT>,
    handler: impl Fn(&mut W, &mut EventCtx, &CT, &mut T) -> bool + 'static,
  ) -> ControllerHost<Self, super::controllers::OnCmd<CT, T, W>> {
    ControllerHost::new(
      self,
      super::controllers::OnCmd::new(
        selector,
        super::controllers::CommandFn::Plain(Box::new(handler)),
      ),
    )
  }

  /**
  Execute closure when command is received, with mutable access to the child
  widget.
  * Must return bool indicating if the event should be propgated to the
  child - true to propagate, false to not.
  */
  fn on_command3<CT: 'static>(
    self,
    selector: Selector<CT>,
    handler: impl Fn(&mut W, &mut EventCtx, &CT, &mut T, &Env) -> bool + 'static,
  ) -> ControllerHost<Self, super::controllers::OnCmd<CT, T, W>> {
    ControllerHost::new(
      self,
      super::controllers::OnCmd::new(
        selector,
        super::controllers::CommandFn::WithEnv(Box::new(handler)),
      ),
    )
  }

  fn link_height_with(
    self,
    height_linker: &mut Option<HeightLinkerShared>,
  ) -> LinkedHeights<T, Self> {
    if let Some(linker) = height_linker {
      LinkedHeights::new(self, linker.clone())
    } else {
      let (widget, linker) = LinkedHeights::new_with_linker(self);
      height_linker.replace(linker);

      widget
    }
  }

  fn link_height_unwrapped(self, height_linker: HeightLinkerShared) -> LinkedHeights<T, Self> {
    LinkedHeights::new(self, height_linker)
  }

  fn on_hover(
    self,
    handler: impl Fn(&mut W, &mut EventCtx, &mut T) -> bool + 'static,
  ) -> ControllerHost<Self, OnHover<T, W>> {
    ControllerHost::new(self, OnHover::new(handler))
  }

  fn with_z_index(self, z_index: u32) -> DelayedPainter<T, Self> {
    DelayedPainter::new(self, z_index)
  }

  fn valign_centre(self) -> Align<T> {
    self.align_vertical(UnitPoint::CENTER)
  }

  fn halign_centre(self) -> Align<T> {
    self.align_horizontal(UnitPoint::CENTER)
  }

  fn prism<U, P: Prism<U, T>>(self, prism: P) -> PrismWrap<Self, P, T> {
    PrismWrap::new(self, prism)
  }

  fn constant<U: Data>(self, constant: T) -> LensWrap<U, T, Constant<T>, Self> {
    self.lens(Constant(constant))
  }

  fn scope<U: Data, In: Fn(U) -> T, L: Lens<T, U>>(
    self,
    make_state: In,
    lens: L,
  ) -> Scope<DefaultScopePolicy<In, LensScopeTransfer<L, U, T>>, Self> {
    Scope::from_lens(make_state, lens, self)
  }

  fn scope_independent<U: Data, In: Fn() -> T + 'static>(
    self,
    make_state: In,
  ) -> LensWrap<
    U,
    (),
    Then<druid::lens::Identity, druid::lens::Unit, U>,
    Scope<DefaultScopePolicy<Box<dyn Fn(()) -> T>, LensScopeTransfer<lens::Unit, (), T>>, Self>,
  > {
    Scope::from_lens(
      Box::new(move |_| make_state()) as Box<dyn Fn(()) -> T>,
      lens::Unit,
      self,
    )
    .lens(lens::Identity.then(lens::Unit))
  }

  fn scope_indie_computed<U: Data, In: Fn(U) -> T + 'static>(
    self,
    make_state: In,
  ) -> impl Widget<U> {
    Scope::from_function(make_state, DummyTransfer::default(), self)
  }

  fn invisible_if(self, func: impl Fn(&T, &Env) -> bool + 'static) -> InvisibleIf<T, Self> {
    InvisibleIf::new(func, self)
  }

  fn invisible(self) -> InvisibleIf<T, Self> {
    InvisibleIf::new(|_, _| true, self)
  }

  fn on_key_up(
    self,
    key: keyboard_types::Key,
    func: impl Fn(&mut EventCtx, &mut T) -> bool + 'static,
  ) -> ControllerHost<Self, OnEvent<T, W>> {
    self.on_event(move |_, ctx, event, data| {
      if let Event::KeyUp(key_event) = event
        && key_event.key == key
      {
        func(ctx, data)
      } else {
        false
      }
    })
  }

  fn suppress_event(
    self,
    matches: impl Fn(&Event) -> bool + 'static,
  ) -> ControllerHost<Self, OnEvent<T, W>> {
    self.on_event(move |_, _, event, _| matches(event))
  }

  fn disabled(self) -> impl Widget<T> {
    self.on_added(|_, ctx, _, _| ctx.set_disabled(true))
  }

  fn in_card(self) -> impl Widget<T> {
    Card::new(self)
  }

  fn in_card_builder(self, builder: CardBuilder) -> impl Widget<T> {
    builder.build(self)
  }

  fn scope_with<In: Data, SWO: Widget<State<T, In>> + 'static>(
    self,
    state: In,
    with: impl FnOnce(LensWrap<State<T, In>, T, state_derived_lenses::outer<T, In>, Self>) -> SWO,
  ) -> impl Widget<T> {
    let inner = self.lens(<State<T, In>>::outer);
    Scope::from_lens(
      move |outer| State {
        outer,
        inner: state.clone(),
      },
      <State<T, In>>::outer,
      with(inner),
    )
  }

  fn mask_default(self) -> Mask<T> {
    Mask::new(self).show_mask(true)
  }

  fn shared_constraint(
    self,
    id: impl Into<ConstraintId<T>>,
    axis: Axis,
  ) -> SharedConstraint<T, Self> {
    SharedConstraint::new(self, id, axis)
  }

  fn in_layout_repeater(self) -> LayoutRepeater<T, Self> {
    LayoutRepeater::new(next_id(), self)
  }

  fn stack_tooltip_custom(self, label: impl Widget<T> + 'static) -> StackTooltip<T> {
    StackTooltip::custom(self, label)
      .with_background_color(druid::Color::TRANSPARENT)
      .with_border_color(druid::Color::TRANSPARENT)
      .with_border_width(0.0)
  }

  fn wrap_with_hover_state<S: HoverState>(self, state: S, set_cursor: bool) -> impl Widget<T> {
    self.scope_with_hover_state(state, set_cursor, |widget| widget)
  }

  fn scope_with_hover_state<S: HoverState, WO: Widget<(T, S)> + 'static>(
    self,
    state: S,
    set_cursor: bool,
    scope: impl FnOnce(Box<dyn Widget<(T, S)>>) -> WO,
  ) -> impl Widget<T> {
    scope(self.lens(lens!((T, S), 0)).boxed()).with_hover_state_opts(state, set_cursor)
  }
}

#[derive(Clone, Data, Lens)]
pub struct State<Outer, Inner> {
  pub outer: Outer,
  pub inner: Inner,
}

impl<T: Data, W: Widget<T> + 'static> WidgetExtEx<T, W> for W {}

pub trait WithHoverState<S: HoverState + Data + Clone, T: Data, W: Widget<(T, S)> + 'static>:
  Widget<(T, S)> + Sized + 'static
{
  fn with_hover_state(self, state: S) -> Box<dyn Widget<T>> {
    self.with_hover_state_opts(state, true)
  }

  fn with_hover_state_opts(self, state: S, set_cursor: bool) -> Box<dyn Widget<T>> {
    const HOVER_STATE_CHANGE: Selector = Selector::new("util.hover_state.change");

    let id = WidgetId::next();

    Scope::from_lens(
      move |data| (data, state.clone()),
      lens!((T, S), 0),
      self
        .on_event(move |_, ctx, event, data| {
          if let druid::Event::MouseMove(_) = event
            && !ctx.is_disabled()
          {
            if set_cursor {
              ctx.set_cursor(&druid::Cursor::Pointer);
            }
            data.1.set(true);
            ctx.request_update();
            ctx.request_paint();
          } else if let druid::Event::Command(cmd) = event
            && cmd.is(HOVER_STATE_CHANGE)
          {
            data.1.set(false);
            if set_cursor {
              ctx.clear_cursor()
            }
          }
          ctx.request_update();
          ctx.request_paint();
          false
        })
        .with_id(id)
        .controller(
          ExtensibleController::new().on_lifecycle(move |_, ctx, event, _, _| {
            if let druid::LifeCycle::HotChanged(false) = event {
              ctx.submit_command(HOVER_STATE_CHANGE.to(id))
            }
          }),
        ),
    )
    .boxed()
  }
}

impl<S: HoverState + Data + Clone, T: Data, W: Widget<(T, S)> + 'static> WithHoverState<S, T, W>
  for W
{
}

pub trait WithHoverIdState<T: Data, W: Widget<(T, SharedIdHoverState)> + 'static>:
  Widget<(T, SharedIdHoverState)> + Sized + 'static
{
  fn with_shared_id_hover_state(self, state: SharedIdHoverState) -> Box<dyn Widget<T>> {
    const HOVER_STATE_CHANGE_FOR_ID: Selector<(WidgetId, bool)> =
      Selector::new("util.hover_state.change");

    let id = state.0;
    Scope::from_lens(
      move |data| (data, state.clone()),
      lens!((T, SharedIdHoverState), 0),
      self
        .on_event(move |_, ctx, event, data| {
          /* if let druid::Event::MouseMove(_) = event
            && !ctx.is_disabled()
            && ctx.is_hot()
          {
            ctx.set_cursor(&druid::Cursor::Pointer);
            data.1.set(true);
            ctx.submit_command(HOVER_STATE_CHANGE_FOR_ID.with((id, true)))
          } else  */
          if let druid::Event::Command(cmd) = event
            && let Some((target, state)) = cmd.get(HOVER_STATE_CHANGE_FOR_ID)
            && *target == id
          {
            data.1.set(*state);
            if *state {
              ctx.set_cursor(&druid::Cursor::Pointer);
            } else {
              ctx.clear_cursor()
            }
          }
          ctx.request_update();
          ctx.request_paint();
          false
        })
        .controller(
          ExtensibleController::new().on_lifecycle(move |_, ctx, event, _, _| {
            if let druid::LifeCycle::HotChanged(state) = event {
              ctx.submit_command(HOVER_STATE_CHANGE_FOR_ID.with((id, *state)))
            }
          }),
        ),
    )
    .boxed()
  }
}

impl<T: Data, W: Widget<(T, SharedIdHoverState)> + 'static> WithHoverIdState<T, W> for W {}

pub struct Button2;

impl Button2 {
  pub fn new<T: Data, W: Widget<T> + 'static>(label: W) -> impl Widget<T> {
    label
      .padding((8., 4.))
      .background(button_painter())
      .controller(HoverController::default())
  }

  pub fn from_label<T: Data>(label: impl Into<LabelText<T>>) -> impl Widget<T> {
    Self::new(Label::wrapped_into(label).with_text_size(18.))
  }
}

/// A bad trait
pub trait Collection<T, U> {
  fn insert(&mut self, item: T);

  fn len(&self) -> usize;

  fn is_empty(&self) -> bool {
    self.len() == 0
  }

  fn drain(&mut self) -> U;
}

impl<A: Clone + Hash + Eq, B, C> Collection<(A, B, C), Vec<(A, B, C)>> for HashMap<A, (A, B, C)> {
  fn insert(&mut self, item: (A, B, C)) {
    HashMap::insert(self, item.0.clone(), item);
  }

  fn len(&self) -> usize {
    self.len()
  }

  fn drain(&mut self) -> Vec<(A, B, C)> {
    self.drain().map(|(_, v)| v).collect()
  }
}

impl Collection<Arc<ModEntry>, Vec<Arc<ModEntry>>> for Vec<Arc<ModEntry>> {
  fn insert(&mut self, item: Arc<ModEntry>) {
    self.push(item);
  }

  fn len(&self) -> usize {
    self.len()
  }

  fn drain(&mut self) -> Vec<Arc<ModEntry>> {
    self.split_off(0)
  }
}

pub struct LoadBalancer<T: Any + Send, DRAIN: Any + Send, SINK: Default + Collection<T, DRAIN>> {
  tx: std::sync::LazyLock<Mutex<Weak<mpsc::UnboundedSender<T>>>>,
  sink: PhantomData<SINK>,
  selector: Selector<DRAIN>,
}

impl<T: Any + Send, U: Any + Send, SINK: Default + Collection<T, U> + Send>
  LoadBalancer<T, U, SINK>
{
  pub const fn new(selector: Selector<U>) -> Self {
    Self {
      tx: std::sync::LazyLock::new(Default::default),
      sink: PhantomData,
      selector,
    }
  }

  pub fn sender(&self, ext_ctx: ExtEventSink) -> Arc<mpsc::UnboundedSender<T>> {
    let mut sender = self.tx.lock().unwrap();
    if let Some(tx) = sender.upgrade() {
      tx
    } else {
      let (tx, mut rx) = mpsc::unbounded_channel::<T>();
      let tx = Arc::new(tx);
      let selector = self.selector;
      tokio::task::spawn(async move {
        let sleep = tokio::time::sleep(std::time::Duration::from_millis(50));
        tokio::pin!(sleep);

        let mut sink = SINK::default();
        loop {
          select! {
            message = rx.recv() => {
              match message {
                Some(message) => {
                  sink.insert(message);
                },
                None => {
                  if !sink.is_empty() {
                    let vals = sink.drain();
                    let _ = ext_ctx.submit_command(selector, vals, Target::Auto);
                  }
                  break
                },
              }
            },
            _ = &mut sleep => {
              let vals = sink.drain();
              let _ = ext_ctx.submit_command(selector, vals, Target::Auto);
              sleep.as_mut().reset(tokio::time::Instant::now() + std::time::Duration::from_millis(50));
            }
          }
        }
      });

      *sender = Arc::downgrade(&tx);

      tx
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Default)]
pub struct FastImMap<K: Clone + Hash + Eq, V: Clone>(druid::im::HashMap<K, V, ahash::RandomState>);

impl<K: Clone + Hash + Eq, V: Clone> FastImMap<K, V> {
  pub fn new() -> Self {
    Self(druid::im::HashMap::with_hasher(ahash::RandomState::new()))
  }

  pub fn inner(self) -> druid::im::HashMap<K, V, ahash::RandomState> {
    self.0
  }
}

impl<K: Clone + Hash + Eq, V: Clone> Debug for FastImMap<K, V>
where
  druid::im::HashMap<K, V, ahash::RandomState>: Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.deref().fmt(f)
  }
}

impl<K: Clone + Hash + Eq, V: Clone + Hash> Hash for FastImMap<K, V>
where
  druid::im::HashMap<K, V, ahash::RandomState>: Hash,
{
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.deref().hash(state)
  }
}

impl<K: Clone + Hash + Eq, V: Clone> Deref for FastImMap<K, V> {
  type Target = druid::im::HashMap<K, V, ahash::RandomState>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<K: Clone + Hash + Eq, V: Clone> DerefMut for FastImMap<K, V> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl<KB: Hash + Eq + ?Sized, K: Clone + Hash + Eq + Borrow<KB>, V: Clone> Index<&KB>
  for FastImMap<K, V>
{
  type Output = V;

  fn index(&self, index: &KB) -> &Self::Output {
    self.deref().index(index)
  }
}

impl<KB: Hash + Eq + ?Sized, K: Clone + Hash + Eq + Borrow<KB>, V: Clone> IndexMut<&KB>
  for FastImMap<K, V>
{
  fn index_mut(&mut self, index: &KB) -> &mut Self::Output {
    self.deref_mut().index_mut(index)
  }
}

impl<K: Clone + Eq + Hash + 'static, V: Clone + Data + 'static> Data for FastImMap<K, V> {
  fn same(&self, other: &Self) -> bool {
    self.is_submap_by(&**other, |other, val| other.same(val))
  }
}

impl<K: Clone + Hash + Eq, V: Clone> From<FastImMap<K, V>>
  for druid::im::HashMap<K, V, ahash::RandomState>
{
  fn from(other: FastImMap<K, V>) -> Self {
    other.0
  }
}

impl<K: Clone + Hash + Eq + PartialEq + Eq, V: Clone, O: Into<druid::im::HashMap<K, V>>> From<O>
  for FastImMap<K, V>
{
  fn from(other: O) -> Self {
    let mut new = Self::new();
    new.extend(other.into().iter().map(|(k, v)| (k.clone(), v.clone())));

    new
  }
}

impl<K: Clone + Hash + Eq, V: Clone> PartialEq for FastImMap<K, V>
where
  druid::im::HashMap<K, V, ahash::RandomState>: PartialEq,
{
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl<K: Clone + Hash + Eq, V: Clone> Eq for FastImMap<K, V> where
  druid::im::HashMap<K, V, ahash::RandomState>: Eq
{
}

pub trait LensExtExt<A: ?Sized, B: ?Sized>: Lens<A, B> {
  fn compute<Get, C>(self, get: Get) -> Then<Self, lens::Map<Get, fn(&mut B, C)>, B>
  where
    Get: Fn(&B) -> C,
    Self: Sized,
  {
    self.map(get, |_, _| {})
  }

  fn cloned(self) -> Then<Self, lens::Map<fn(&B) -> B, fn(&mut B, B)>, B>
  where
    Self: Sized,
    B: Clone,
  {
    self.map(|a| a.clone(), |b, a| *b = a)
  }

  fn owned<C>(self) -> Then<Self, lens::Map<fn(&B) -> C, fn(&mut B, C)>, B>
  where
    Self: Sized,
    B: ToOwned<Owned = C> + Clone,
    C: Borrow<B> + Clone,
  {
    self.map(|b| b.to_owned(), |b, c| b.clone_from(c.borrow()))
  }

  fn debug<DBG>(self, dbg: DBG) -> Then<Self, Dbg<DBG>, B>
  where
    Self: Sized,
    DBG: Fn(&B) + 'static,
    B: Clone,
  {
    self.then(Dbg(dbg))
  }
}

impl<A: ?Sized, B: ?Sized, T: Lens<A, B>> LensExtExt<A, B> for T {}

#[derive(Clone)]
pub struct Compute<Get: Fn(&B) -> C, B: ?Sized, C>(lens::Map<Get, fn(&mut B, C)>);

impl<Get: Fn(&B) -> C, B: ?Sized, C> Compute<Get, B, C> {
  pub fn new(f: Get) -> Self {
    Self(lens::Map::new(f, |_, _| {}))
  }
}

impl<Get: Fn(&B) -> C, B: ?Sized, C> Lens<B, C> for Compute<Get, B, C> {
  fn with<V, F: FnOnce(&C) -> V>(&self, data: &B, f: F) -> V {
    self.0.with(data, f)
  }

  fn with_mut<V, F: FnOnce(&mut C) -> V>(&self, data: &mut B, f: F) -> V {
    self.0.with_mut(data, f)
  }
}

pub struct Dbg<DBG>(DBG);

impl<T, DBG: Fn(&T) + 'static> Lens<T, T> for Dbg<DBG> {
  fn with<V, F: FnOnce(&T) -> V>(&self, data: &T, f: F) -> V {
    self.0(data);
    f(data)
  }

  fn with_mut<V, F: FnOnce(&mut T) -> V>(&self, data: &mut T, f: F) -> V {
    self.0(data);
    f(data)
  }
}

pub trait PrismExt<A, B>: Prism<A, B> {
  fn then_some<Other, C>(self, right: Other) -> ThenSome<Self, Other, B>
  where
    Other: Prism<B, C>,
    Self: Sized,
  {
    ThenSome::new(self, right)
  }
}

impl<A, B, T: Prism<A, B>> PrismExt<A, B> for T {}

#[derive(Clone)]
pub struct ThenSome<T, U, B> {
  left: T,
  right: U,
  _marker: PhantomData<B>,
}

impl<T, U, B> ThenSome<T, U, B> {
  pub fn new<A, C>(left: T, right: U) -> Self
  where
    T: Prism<A, B>,
    U: Prism<B, C>,
  {
    Self {
      left,
      right,
      _marker: PhantomData,
    }
  }
}

impl<T, U, A, B, C> Prism<A, C> for ThenSome<T, U, B>
where
  T: Prism<A, B>,
  U: Prism<B, C>,
{
  fn get(&self, data: &A) -> Option<C> {
    self.left.get(data).and_then(|b| self.right.get(&b))
  }

  fn put(&self, data: &mut A, inner: C) {
    let temp: Option<B> = self.left.get(data);
    if let Some(mut temp) = temp {
      self.right.put(&mut temp, inner);
      self.left.put(data, temp);
    }
  }
}

pub struct IsSome;

impl IsSome {
  pub fn new<B, F: Fn(&B) -> Option<B>>(func: F) -> Closures<F, fn(&mut B, B)> {
    Closures(func, |a, b| *a = b)
  }
}

pub fn option_ptr_cmp<T>(this: &Option<Rc<T>>, other: &Option<Rc<T>>) -> bool {
  if let Some(this) = this
    && let Some(other) = other
  {
    Rc::ptr_eq(this, other)
  } else {
    false
  }
}

pub trait ShadeColor {
  fn lighter(self) -> Self;

  fn lighter_by(self, mult: usize) -> Self;

  fn darker(self) -> Self;

  fn darker_by(self, mult: usize) -> Self;

  fn interpolate_with(self, other: Self, mult: usize) -> Self;
}

impl ShadeColor for Color {
  fn lighter(self) -> Self {
    self.interpolate(&Color::WHITE, 1.0 / 16.0)
  }

  fn lighter_by(self, mult: usize) -> Self {
    self.interpolate(&Color::WHITE, mult as f64 / 16.0)
  }

  fn darker(self) -> Self {
    self.interpolate(&Color::BLACK, 1.0 / 16.0)
  }

  fn darker_by(self, mult: usize) -> Self {
    self.interpolate(&Color::BLACK, mult as f64 / 16.0)
  }

  fn interpolate_with(self, other: Self, mult: usize) -> Self {
    self.interpolate(&other, mult as f64 / 16.)
  }
}

pub struct PrismBox<T, U>(Box<dyn Prism<T, U>>);

impl<T, U> PrismBox<T, U> {
  pub fn new(prism: impl Prism<T, U> + 'static) -> Self {
    Self(Box::new(prism))
  }
}

impl<T, U> Prism<T, U> for PrismBox<T, U> {
  fn get(&self, data: &T) -> Option<U> {
    self.0.get(data)
  }

  fn put(&self, data: &mut T, inner: U) {
    self.0.put(data, inner)
  }
}

mod modname {}

#[extend::ext(name = Tap)]
pub impl<T> T {
  fn tap<U>(mut self, func: impl FnOnce(&mut Self) -> U) -> Self {
    func(&mut self);
    self
  }

  fn pipe<U>(self, func: impl FnOnce(Self) -> U) -> U
  where
    Self: Sized,
  {
    func(self)
  }
}

#[derive(Debug, Clone)]
pub struct DataTimer(TimerToken);

impl PartialEq<TimerToken> for DataTimer {
  fn eq(&self, other: &TimerToken) -> bool {
    &self.0 == other
  }
}

impl DataTimer {
  pub const INVALID: Self = Self(TimerToken::INVALID);
}

impl Data for DataTimer {
  fn same(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl Deref for DataTimer {
  type Target = TimerToken;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<TimerToken> for DataTimer {
  fn from(value: TimerToken) -> Self {
    Self(value)
  }
}

#[macro_export]
macro_rules! match_command {
  ($val:expr, $default:expr => {$($($selector:ident)::* $(($bind:ident))? => $body:expr),+ $(,)? }) => {
    match $val {
      val => match () {
        $(
          () if val.is($($selector)::*) => {
            let _selector = $($selector)::*;
            $(let $bind = val.get_unchecked(_selector);)?
            $body
          }
        )+
        _ => {
          $default
        }
      }
    }
  };
  ($val:expr, $default:expr => {$($($($selector:ident)::*, )+ => $body:expr),+ $(,)? }) => {
    match $val {
      val => match () {
        $(
          $(
            () if val.is($($selector)::*) => {
              $body
            }
          )+
        )+
        _ => {
          $default
        }
      }
    }
  };
}

#[extend::ext(name = PrintAndPanic)]
pub impl<T, E: Debug> Result<T, E> {
  fn inspanic(self, msg: &str) {
    self.inspect_err(|e| eprintln!("{e:?}")).expect(msg);
  }
}

pub struct ValueFormatter;

impl druid::text::Formatter<u32> for ValueFormatter {
  fn format(&self, value: &u32) -> String {
    value.to_string()
  }

  fn validate_partial_input(
    &self,
    input: &str,
    _sel: &druid::text::Selection,
  ) -> druid::text::Validation {
    match input.parse::<u32>() {
      Err(err) if !input.is_empty() => druid::text::Validation::failure(err),
      _ => druid::text::Validation::success(),
    }
  }

  fn value(&self, input: &str) -> Result<u32, druid::text::ValidationError> {
    input
      .parse::<u32>()
      .map_err(druid::text::ValidationError::new)
  }
}

pub fn ident_arc<T: Data>() -> lens::InArc<lens::Identity> {
  lens::InArc::new::<T, T>(lens::Identity)
}

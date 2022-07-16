use std::marker::PhantomData;
use std::{io::Read, sync::Arc, path::PathBuf, collections::VecDeque};

use druid::{MouseEvent, Env};
use druid::widget::{ControllerHost, LabelText};
use druid::{
  widget::{
    Label, LensWrap, Flex, Axis, RawLabel, Controller, ScopeTransfer, Painter, Scope
  },
  text::{
    RichText, AttributeSpans, Attribute
  },
  Data, Lens, WidgetExt, Widget, ExtEventSink, Selector, Target, lens,
  FontWeight, Key, Color, KeyOrValue, Point, EventCtx, Event, theme,
  RenderContext, UnitPoint, Command
};
use druid_widget_nursery::CommandCtx;
use if_chain::if_chain;
use json_comments::strip_comments;
use serde::Deserialize;
use tap::Tap;
use lazy_static::lazy_static;
use regex::Regex;

use crate::patch::click::Click;

use super::controllers::{OnNotif, HoverController, OnEvent};
use super::mod_entry::{ModVersionMeta, GameVersion};

pub(crate) mod icons;

pub use icons::*;

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

#[derive(Debug, Clone)]
pub enum LoadError {
  NoSuchFile,
  ReadError,
  FormatError
}

#[derive(Debug, Clone)]
pub enum SaveError {
  File,
  Write,
  Format,
}

pub fn get_quoted_version(starsector_version: &(Option<String>, Option<String>, Option<String>, Option<String>)) -> Option<String> {
  match starsector_version {
    (None, None, None, None) => None,
    (major, minor, patch, rc) => {
      Some(format!(
        "{}.{}{}{}",
        major.clone().unwrap_or_else(|| "0".to_string()),
        minor.clone().unwrap_or_else(|| "".to_string()),
        patch.clone().map_or_else(|| "".to_string(), |p| format!(".{}", p)),
        rc.clone().map_or_else(|| "".to_string(), |rc| format!("a-RC{}", rc))
      ))
    }
  }
}

pub trait LabelExt<T: Data> {
  fn wrapped(label: &str) -> Label<T> {
    Label::new(label).with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }
  
  fn wrapped_lens<U: Data, L: Lens<T, U>>(lens: L) -> LensWrap<T, String, L, Label<String>> {
    LensWrap::new(Label::dynamic(|t: &String, _| t.to_string()).with_line_break_mode(druid::widget::LineBreaking::WordWrap), lens)
  }

  fn wrapped_func<F, S>(func: F) -> Label<T>
  where 
    S: Into<Arc<str>>,
    F: Fn(&T, &druid::Env) -> S + 'static
  {
    Label::new(func).with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }

  fn wrapped_into(label: impl Into<LabelText<T>>) -> Label<T> {
    Label::new(label).with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }
}

impl<T: Data> LabelExt<T> for Label<T> {}

pub fn make_flex_pair<T: Data>(label: impl Widget<T> + 'static, ratio_1: f64, val: impl Widget<T> + 'static, ratio_2: f64, axis: Axis) -> Flex<T> {
  Flex::for_axis(axis)
    .with_flex_child(label.expand_width(), ratio_1)
    .with_flex_child(val.expand_width(), ratio_2)
}

pub fn make_flex_description_row<T: Data>(label: impl Widget<T> + 'static, val: impl Widget<T> + 'static) -> Flex<T> {
  make_flex_pair(label, 1., val, 1.5, Axis::Horizontal)
}

pub fn make_flex_settings_row<T: Data>(widget: impl Widget<T> + 'static, label: impl Widget<T> + 'static) -> Flex<T> {
  make_flex_pair(widget.align_horizontal(UnitPoint::CENTER), 1., label, 10., Axis::Horizontal)
}

pub fn make_flex_column_pair<T: Data>(label: impl Widget<T> + 'static, val: impl Widget<T> + 'static) -> Flex<T> {
  make_flex_pair(label, 1., val, 1., Axis::Vertical)
}

pub fn make_pair<T: Data>(label: impl Widget<T> + 'static, val: impl Widget<T> + 'static, axis: Axis) -> Flex<T> {
  Flex::for_axis(axis)
    .with_child(label.expand_width())
    .with_child(val.expand_width())
}

pub fn make_description_row<T: Data>(label: impl Widget<T> + 'static, val: impl Widget<T> + 'static) -> Flex<T> {
  make_pair(label, val, Axis::Horizontal)
}

pub fn make_column_pair<T: Data>(label: impl Widget<T> + 'static, val: impl Widget<T> + 'static) -> Flex<T> {
  make_pair(label, val, Axis::Vertical)
}

pub const MASTER_VERSION_RECEIVED: Selector<(String, Result<ModVersionMeta, String>)> = Selector::new("remote_version_received");

pub async fn get_master_version(ext_sink: ExtEventSink, local: ModVersionMeta) {
  let res = send_request(local.remote_url.clone()).await;

  let payload = match res {
    Err(err) => (local.id.clone(), Err(err)),
    Ok(remote) => {
      if_chain! {
        let mut stripped = String::new();
        if strip_comments(remote.as_bytes()).read_to_string(&mut stripped).is_ok();
        if let Ok(normalized) = handwritten_json::normalize(&stripped);
        if let Ok(remote) = json5::from_str::<ModVersionMeta>(&normalized);
        then {
          (
            local.id.clone(),
            Ok(remote)
          )
        } else {
          (
            local.id.clone(),
            Err(format!("Parse error. Payload:\n{}", remote))
          )
        }
      }
    }
  };

  if let Err(err) = ext_sink.submit_command(MASTER_VERSION_RECEIVED, payload, Target::Auto) {
    eprintln!("Failed to submit remote version data {}", err)
  };
}

async fn send_request(url: String) -> Result<String, String>{
  reqwest::get(url)
    .await
    .map_err(|e| format!("{:?}", e))?
    .error_for_status()
    .map_err(|e| format!("{:?}", e))?
    .text()
    .await
    .map_err(|e| format!("{:?}", e))
}

pub fn bold_text<T: Data>(text: &str, size: impl Into<KeyOrValue<f64>>, weight: FontWeight, colour: impl Into<KeyOrValue<Color>>) -> impl Widget<T> {
  RawLabel::new()
    .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
    .lens(lens::Constant(RichText::new_with_attributes(
    text.into(),
    AttributeSpans::new().tap_mut(|s| {
      s.add(0..text.len(), Attribute::Weight(weight));
      s.add(0..text.len(), Attribute::FontSize(size.into()));
      s.add(0..text.len(), Attribute::TextColor(colour.into()));
    })
  )))
}

pub fn h1<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(text, 24., FontWeight::BOLD, theme::TEXT_COLOR)
}

pub fn h2<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(text, 20., FontWeight::SEMI_BOLD, theme::TEXT_COLOR)
}

pub fn h3<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(text, 18., FontWeight::MEDIUM, theme::TEXT_COLOR)
}

pub const GET_INSTALLED_STARSECTOR: Selector<Result<GameVersion, LoadError>> = Selector::new("util.starsector_version.get");

pub async fn get_starsector_version(ext_ctx: ExtEventSink, install_dir: PathBuf) {
  use classfile_parser::class_parser;
  use tokio::{task, fs};
  use regex::bytes::Regex;

  #[cfg(target_os = "linux")]
  let obf_jar = install_dir.join("starfarer_obf.jar");
  #[cfg(target_os = "windows")]
  let obf_jar = install_dir.join("starsector-core/starfarer_obf.jar");
  #[cfg(target_os = "macos")]
  let obf_jar = install_dir.join("Contents/Resources/Java/starfarer_obf.jar");

  let mut res = task::spawn_blocking(move || {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(obf_jar).unwrap()).unwrap();

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
              Some(utf_const.utf8_string.clone())
            } else {
              None
            }
          }
        }).ok_or(LoadError::FormatError)
      })
  }).await
  .map_err(|_| LoadError::ReadError)
  .flatten();

  if res.is_err() {
    lazy_static! {
      static ref RE: Regex = Regex::new(r"Starting Starsector (.*) launcher").unwrap();
    }
    res = fs::read(install_dir.join("starsector-core").join("starsector.log")).await
      .map_err(|_| LoadError::ReadError)
      .and_then(|file| {
        RE.captures(&file)
          .and_then(|captures| captures.get(1))
          .ok_or(LoadError::FormatError)
          .and_then(|m| String::from_utf8(m.as_bytes().to_vec()).map_err(|_| LoadError::FormatError))
      })
  };

  let parsed = res.map(|text| parse_game_version(&text));

  if ext_ctx.submit_command(GET_INSTALLED_STARSECTOR, parsed, Target::Auto).is_err() {
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
pub fn parse_game_version(text: &str) -> (Option<String>, Option<String>, Option<String>, Option<String>) {
  lazy_static! {
    static ref VERSION_REGEX: Regex = Regex::new(r"\.|a-RC|A-RC|a-rc|a").unwrap();
  }
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

pub enum StarsectorVersionDiff {
  Major,
  Minor,
  Patch,
  RC,
  None
}

impl From<(&GameVersion, &GameVersion)> for StarsectorVersionDiff {
  fn from(vals: (&GameVersion, &GameVersion)) -> Self {
    match vals {
      ((mod_major, ..), (game_major, ..)) if mod_major != game_major => {
        StarsectorVersionDiff::Major
      },
      ((_, mod_minor, ..), (_, game_minor, ..)) if mod_minor != game_minor => {
        StarsectorVersionDiff::Minor
      },
      ((.., mod_patch, _), (.., game_patch, _)) if mod_patch != game_patch => {
        StarsectorVersionDiff::Patch
      },
      ((.., mod_rc), (.., game_rc)) if mod_rc != game_rc => {
        StarsectorVersionDiff::RC
      },
      _ => {
        StarsectorVersionDiff::None
      }
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
      StarsectorVersionDiff::None => GREEN_KEY.into()
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
  pub assets: Vec<Asset>
}

#[derive(Deserialize, Clone)]
pub struct Asset {
  pub name: String,
  pub browser_download_url: String
}

pub async fn get_latest_manager() -> Result<Release, String> {
  let client = reqwest::Client::builder()
    .user_agent("StarsectorModManager")
    .build()
    .map_err(|e| e.to_string())?;

  let mut res = client.get("https://api.github.com/repos/atlanticaccent/starsector-mod-manager-rust/releases")
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

pub fn default_true() -> bool { true }

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

pub struct Card;

impl Card {
  const CARD_INSET: f64 = 12.5;

  pub fn new<T: Data>(widget: impl Widget<T> + 'static) -> impl Widget<T> {
    widget
      .padding((Self::CARD_INSET, Self::CARD_INSET, Self::CARD_INSET, Self::CARD_INSET + 5.))
      .background(Self::card_painter())
  }

  pub fn card_painter<T: Data>() -> Painter<T> {
    Painter::new(|ctx, _, env| {
      let size = ctx.size();
  
      let rounded_rect = size
        .to_rect()
        .inset(-Self::CARD_INSET / 2.0)
        .to_rounded_rect(10.);
  
      ctx.fill(rounded_rect, &env.get(theme::BACKGROUND_LIGHT));
    })
  }
}

pub trait CommandExt: CommandCtx {
  fn submit_command_global(&mut self, cmd: impl Into<Command>) {
    let cmd: Command = cmd.into();
    self.submit_command(cmd.to(Target::Global))
  }
}

impl<T: CommandCtx> CommandExt for T {}

#[derive(Default)]
pub struct DummyTransfer<X, Y> {
  phantom_x: PhantomData<X>,
  phantom_y: PhantomData<Y>
}

impl<X: Data, Y: Data> ScopeTransfer for DummyTransfer<X, Y> {
  type In = X;
  type State = Y;

  fn read_input(&self, _: &mut Self::State, _: &Self::In) {}

  fn write_back_input(&self, _: &Self::State, _: &mut Self::In) {}
}

pub fn hoverable_text(colour: Option<Color>) -> impl Widget<String> {
  struct TextHoverController;

  impl<D: Data, W: Widget<(D, bool)>> Controller<(D, bool), W> for TextHoverController {
    fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut (D, bool), env: &druid::Env) {
      if let Event::MouseMove(_) = event {
        data.1 = ctx.is_hot() && !ctx.is_disabled()
      }

      child.event(ctx, event, data, env)
    }
  }

  Scope::from_function(
    |input: String| (input, false),
    DummyTransfer::default(),
    RawLabel::new()
      .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
      .lens(lens::Map::new(
        move |(text, hovered): &(String, bool)| RichText::new(text.clone().into())
          .with_attribute(0..text.len(), Attribute::Underline(*hovered))
          .with_attribute(0..text.len(), Attribute::TextColor(colour.clone().map(|c| c.into()).unwrap_or_else(|| theme::TEXT_COLOR.into()))),
          |_, _| {}
      ))
      .controller(TextHoverController)
  )
}

pub trait WidgetExtEx<T: Data>: Widget<T> + Sized + 'static {
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

  fn on_event(
    self,
    f: impl Fn(&mut EventCtx, &Event, &mut T) -> bool + 'static,
  ) -> ControllerHost<Self, OnEvent<T>> {
    ControllerHost::new(self, OnEvent::new(f))
  }
}

impl<T: Data, W: Widget<T> + 'static> WidgetExtEx<T> for W {}

pub struct Button2;

impl Button2 {
  pub fn new<T: Data, W: Widget<T> + 'static>(label: W) -> impl Widget<T> {
    label
      .padding((8., 4.))
      .background(button_painter())
      .controller(HoverController)
  }

  pub fn from_label<T: Data>(label: impl Into<LabelText<T>>) -> impl Widget<T> {
    Self::new(
      Label::wrapped_into(label)
        .with_text_size(18.)
    )
  }
}

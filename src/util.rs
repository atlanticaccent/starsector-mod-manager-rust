use std::{io::Read, sync::Arc, path::PathBuf};

use druid::{widget::{Label, LensWrap, Flex, Axis, RawLabel, Controller}, Data, Lens, WidgetExt, Widget, ExtEventSink, Selector, Target, lens, text::{RichText, AttributeSpans, Attribute}, FontWeight, Key, Color, KeyOrValue, Point, EventCtx, Event};
use if_chain::if_chain;
use json_comments::strip_comments;
use tap::Tap;
use lazy_static::lazy_static;
use regex::Regex;

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
    Err(err) => (local.id, Err(err)),
    Ok(remote) => {
      if_chain! {
        let mut stripped = String::new();
        if strip_comments(remote.as_bytes()).read_to_string(&mut stripped).is_ok();
        if let Ok(normalized) = handwritten_json::normalize(&stripped);
        if let Ok(remote) = json5::from_str::<ModVersionMeta>(&normalized);
        then {
          (
            local.id,
            Ok(remote)
          )
        } else {
          (
            local.id,
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

pub fn bold_header<T: Data>(text: &str, size: f64, weight: FontWeight) -> impl Widget<T> {
  RawLabel::new()
    .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
    .lens(lens::Constant(RichText::new_with_attributes(
    text.into(),
    AttributeSpans::new().tap_mut(|s| {
      s.add(0..text.len(), Attribute::Weight(weight));
      s.add(0..text.len(), Attribute::FontSize(size.into()))
    })
  )))
}

pub fn h1<T: Data>(text: &str) -> impl Widget<T> {
  bold_header(text, 24., FontWeight::BOLD)
}

pub fn h2<T: Data>(text: &str) -> impl Widget<T> {
  bold_header(text, 20., FontWeight::SEMI_BOLD)
}

pub fn h3<T: Data>(text: &str) -> impl Widget<T> {
  bold_header(text, 18., FontWeight::MEDIUM)
}

pub const GET_INSTALLED_STARSECTOR: Selector<Result<GameVersion, LoadError>> = Selector::new("util.starsector_version.get");

pub async fn get_starsector_version(ext_ctx: ExtEventSink, install_dir: PathBuf) {
  use classfile_parser::class_parser;
  use tokio::{task, fs};
  use regex::bytes::Regex;

  let install_dir_clone = install_dir.clone();
  let mut res = task::spawn_blocking(move || {
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

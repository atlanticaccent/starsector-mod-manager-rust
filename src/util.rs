use std::io::Read;

use druid::{widget::{Label, LensWrap, Flex}, Data, Lens, WidgetExt, Widget, ExtEventSink, Selector, Target};
use if_chain::if_chain;
use json_comments::strip_comments;

use super::mod_entry::ModVersionMeta;

#[derive(Debug, Clone)]
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

pub fn get_game_version(starsector_version: &(Option<String>, Option<String>, Option<String>, Option<String>)) -> Option<String> {
  match starsector_version {
    (None, None, None, None) => None,
    (major, minor, patch, rc) => {
      Some(format!(
        "{}.{}{}{}",
        major.clone().unwrap_or("0".to_string()),
        minor.clone().unwrap_or("".to_string()),
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
}

impl<T: Data> LabelExt<T> for Label<T> {}

pub fn make_description_row<T: Data>(label: impl Widget<T> + 'static, val: impl Widget<T> + 'static) -> impl Widget<T> {
  Flex::row()
    .with_flex_child(label.expand_width(), 1.)
    .with_flex_child(val.expand_width(), 1.5)
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

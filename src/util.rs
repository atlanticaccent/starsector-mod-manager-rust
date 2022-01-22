use druid::{widget::{Label, LensWrap, LabelText, Flex}, Data, Lens, WidgetExt, Widget};

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
    .with_flex_child(val.expand_width(), 2.)
    .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
}

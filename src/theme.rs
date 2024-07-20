use druid::{piet::ColorParseError, theme, Color, Data, Env, Key, Selector};
use druid_widget_nursery::prism::Prism;
use serde::{Deserialize, Serialize};

use crate::app::util;

pub const CHANGE_THEME: Selector<Theme> = Selector::new("app.theme.change");
pub const SHADOW: Key<Color> = Key::new("custom_theme.shadow");

#[derive(Data, Clone)]
pub struct Theme {
  text: Option<Color>,
  button_dark: Option<Color>,
  button_light: Option<Color>,
  background_dark: Color,
  background_light: Color,
  border_dark: Color,
  border_light: Color,
  action: Color,
  action_text: Color,
  success: Color,
  success_text: Color,
  error: Color,
  error_text: Color,
  warning: Color,
  warning_text: Color,
  do_not_ignore: Color,
  do_not_ignore_text: Color,
  shadow: Option<Color>,
}

impl Theme {
  pub const LEGACY: Self = Self {
    text: None,
    button_dark: None,
    button_light: None,
    background_dark: unwrap(Color::from_hex_str("#1f1a1b")),
    background_light: unwrap(Color::from_hex_str("#292425")),
    border_dark: unwrap(Color::from_hex_str("#48454f")),
    border_light: unwrap(Color::from_hex_str("#c9c4cf")),
    action: unwrap(Color::from_hex_str("#004d66")),
    action_text: unwrap(Color::from_hex_str("#bbe9ff")),
    success: unwrap(Color::from_hex_str("#135200")),
    success_text: unwrap(Color::from_hex_str("#adf68a")),
    error: unwrap(Color::from_hex_str("#930006")),
    error_text: unwrap(Color::from_hex_str("#ffdad4")),
    warning: unwrap(Color::from_hex_str("#574500")),
    warning_text: unwrap(Color::from_hex_str("#ffe174")),
    do_not_ignore: unwrap(Color::from_hex_str("#7f2c00")),
    do_not_ignore_text: unwrap(Color::from_hex_str("#ffdbcc")),
    shadow: None,
  };

  pub const RETRO: Self = Self {
    text: Some(unwrap(Color::from_hex_str("#101422"))),
    button_dark: Some(unwrap(Color::from_hex_str("#e6e6e6"))),
    button_light: Some(unwrap(Color::from_hex_str("#e6e6e6"))),
    background_dark: unwrap(Color::from_hex_str("#ccc3a4")),
    background_light: unwrap(Color::from_hex_str("#efebdd")),
    border_dark: unwrap(Color::from_hex_str("#101422")),
    border_light: unwrap(Color::from_hex_str("#161a28")),
    action: unwrap(Color::from_hex_str("#004d66")),
    action_text: unwrap(Color::from_hex_str("#bbe9ff")),
    success: unwrap(Color::from_hex_str("#135200")),
    success_text: unwrap(Color::from_hex_str("#adf68a")),
    error: unwrap(Color::from_hex_str("#930006")),
    error_text: unwrap(Color::from_hex_str("#ffdad4")),
    warning: unwrap(Color::from_hex_str("#574500")),
    warning_text: unwrap(Color::from_hex_str("#ffe174")),
    do_not_ignore: unwrap(Color::from_hex_str("#7f2c00")),
    do_not_ignore_text: unwrap(Color::from_hex_str("#ffdbcc")),
    shadow: None,
  };

  pub fn apply(self, env: &mut Env) {
    let Self {
      text,
      button_dark,
      button_light,
      background_dark,
      background_light,
      border_dark,
      border_light,
      action,
      action_text,
      success,
      success_text,
      error,
      error_text,
      warning,
      warning_text,
      do_not_ignore,
      do_not_ignore_text,
      shadow,
    } = self;

    if let Some(text) = text {
      env.set(theme::TEXT_COLOR, text)
    }
    if let Some(button_dark) = button_dark {
      env.set(theme::BUTTON_DARK, button_dark);
    }
    if let Some(button_light) = button_light {
      env.set(theme::BUTTON_LIGHT, button_light);
    }
    env.set(theme::BACKGROUND_DARK, background_dark);
    env.set(theme::BACKGROUND_LIGHT, background_light);
    env.set(
      theme::WINDOW_BACKGROUND_COLOR,
      env.get(theme::BACKGROUND_DARK),
    );
    env.set(theme::BORDER_DARK, border_dark);
    env.set(theme::BORDER_LIGHT, border_light);
    env.set(util::BLUE_KEY, action);
    env.set(util::ON_BLUE_KEY, action_text);
    env.set(util::GREEN_KEY, success);
    env.set(util::ON_GREEN_KEY, success_text);
    env.set(util::RED_KEY, error);
    env.set(util::ON_RED_KEY, error_text);
    env.set(util::YELLOW_KEY, warning);
    env.set(util::ON_YELLOW_KEY, warning_text);
    env.set(util::ORANGE_KEY, do_not_ignore);
    env.set(util::ON_ORANGE_KEY, do_not_ignore_text);
    if let Some(shadow) = shadow {
      env.set(SHADOW, shadow)
    }
  }
}

const fn unwrap(res: Result<Color, ColorParseError>) -> Color {
  match res {
    Ok(color) => color,
    Err(_) => panic!(),
  }
}

#[derive(
  Serialize,
  Deserialize,
  Clone,
  Copy,
  PartialEq,
  Eq,
  Default,
  strum_macros::AsRefStr,
  strum_macros::EnumIter,
  Prism,
  Data,
  Debug,
)]
pub enum Themes {
  #[default]
  Retro,
  Legacy,
  Troutman,
  Gates,
  #[strum(serialize = "Ward Two")]
  WardTwo
}

impl From<Themes> for Theme {
  fn from(theme: Themes) -> Self {
    match theme {
      Themes::Retro => Self::RETRO,
      Themes::Legacy => Self::LEGACY,
      Themes::Troutman => TROUTMANST_BROOKLYN,
      Themes::Gates => GATESAVE_BROOKLYN,
      Themes::WardTwo => WARDTWO_MA,
    }
  }
}

// https://www.dayroselane.com/hydrants/details/40_704692_-73_924656
const TROUTMANST_BROOKLYN: Theme = Theme {
  text: Some(unwrap(Color::from_hex_str("#36717e"))),
  button_dark: Some(unwrap(Color::from_hex_str("#7b474a"))),
  button_light: Some(unwrap(Color::from_hex_str("#7b474a"))),
  background_dark: unwrap(Color::from_hex_str("#050405")),
  background_light: unwrap(Color::from_hex_str("#2f2e2f")),
  border_dark: unwrap(Color::from_hex_str("#4a6d92")),
  border_light: unwrap(Color::from_hex_str("#7f5f5e")),
  action: unwrap(Color::from_hex_str("#004d66")),
  action_text: unwrap(Color::from_hex_str("#bbe9ff")),
  success: unwrap(Color::from_hex_str("#135200")),
  success_text: unwrap(Color::from_hex_str("#adf68a")),
  error: unwrap(Color::from_hex_str("#930006")),
  error_text: unwrap(Color::from_hex_str("#ffdad4")),
  warning: unwrap(Color::from_hex_str("#574500")),
  warning_text: unwrap(Color::from_hex_str("#ffe174")),
  do_not_ignore: unwrap(Color::from_hex_str("#7f2c00")),
  do_not_ignore_text: unwrap(Color::from_hex_str("#ffdbcc")),
  shadow: Some(unwrap(Color::from_hex_str("#d0c5a7"))),
};

// https://www.dayroselane.com/hydrants/details/40_699769_-73_912078
const GATESAVE_BROOKLYN: Theme = Theme {
  text: Some(unwrap(Color::from_hex_str("#c5ac76"))),
  button_dark: Some(unwrap(Color::from_hex_str("#4d6263"))),
  button_light: Some(unwrap(Color::from_hex_str("#4d6263"))),
  background_dark: unwrap(Color::from_hex_str("#2e3939")),
  background_light: unwrap(Color::from_hex_str("#472a2a")),
  border_dark: unwrap(Color::from_hex_str("#060708")),
  border_light: unwrap(Color::from_hex_str("#472a2a")),
  action: unwrap(Color::from_hex_str("#004d66")),
  action_text: unwrap(Color::from_hex_str("#bbe9ff")),
  success: unwrap(Color::from_hex_str("#135200")),
  success_text: unwrap(Color::from_hex_str("#adf68a")),
  error: unwrap(Color::from_hex_str("#930006")),
  error_text: unwrap(Color::from_hex_str("#ffdad4")),
  warning: unwrap(Color::from_hex_str("#574500")),
  warning_text: unwrap(Color::from_hex_str("#ffe174")),
  do_not_ignore: unwrap(Color::from_hex_str("#7f2c00")),
  do_not_ignore_text: unwrap(Color::from_hex_str("#ffdbcc")),
  shadow: Some(unwrap(Color::from_hex_str("#d0c5a7"))),
};

// https://www.dayroselane.com/hydrants/details/42_377487_-71_101188
const WARDTWO_MA: Theme = Theme {
  text: Some(unwrap(Color::from_hex_str("#a47260"))),
  button_dark: Some(unwrap(Color::from_hex_str("#444846"))),
  button_light: Some(unwrap(Color::from_hex_str("#444846"))),
  background_dark: unwrap(Color::from_hex_str("#350f0f")),
  background_light: unwrap(Color::from_hex_str("#443530")),
  border_dark: unwrap(Color::from_hex_str("#443530")),
  border_light: unwrap(Color::from_hex_str("#9a5d41")),
  action: unwrap(Color::from_hex_str("#004d66")),
  action_text: unwrap(Color::from_hex_str("#bbe9ff")),
  success: unwrap(Color::from_hex_str("#135200")),
  success_text: unwrap(Color::from_hex_str("#adf68a")),
  error: unwrap(Color::from_hex_str("#930006")),
  error_text: unwrap(Color::from_hex_str("#ffdad4")),
  warning: unwrap(Color::from_hex_str("#574500")),
  warning_text: unwrap(Color::from_hex_str("#ffe174")),
  do_not_ignore: unwrap(Color::from_hex_str("#7f2c00")),
  do_not_ignore_text: unwrap(Color::from_hex_str("#ffdbcc")),
  shadow: Some(unwrap(Color::from_hex_str("#b08584"))),
};

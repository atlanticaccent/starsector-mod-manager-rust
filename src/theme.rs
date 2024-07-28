use std::{fmt::Display, ops::Deref, str::FromStr};

use druid::{text::FontWeight, theme, Color, Data, Env, Key, Lens, Selector};
use druid_widget_nursery::prism::Prism;
use fake::Fake;
use serde::{de::Visitor, Deserialize, Serialize};

use crate::app::util;

pub const CHANGE_THEME: Selector<Themes> = Selector::new("app.theme.change");
pub const SHADOW: Key<Color> = Key::new("custom_theme.shadow");

#[derive(Debug, Data, Lens, Clone, Serialize, Deserialize)]
pub struct Theme {
  text: Option<ExtColor>,
  button_dark: Option<ExtColor>,
  button_light: Option<ExtColor>,
  background_dark: ExtColor,
  background_light: ExtColor,
  border_dark: ExtColor,
  border_light: ExtColor,
  action: Option<ExtColor>,
  action_text: Option<ExtColor>,
  success: Option<ExtColor>,
  success_text: Option<ExtColor>,
  error: Option<ExtColor>,
  error_text: Option<ExtColor>,
  warning: Option<ExtColor>,
  warning_text: Option<ExtColor>,
  do_not_ignore: Option<ExtColor>,
  do_not_ignore_text: Option<ExtColor>,
  shadow: Option<ExtColor>,
}

#[derive(Debug, Clone, Data)]
pub struct ExtColor(Color);

impl Default for ExtColor {
  fn default() -> Self {
    Self(Color::WHITE)
  }
}

impl Deref for ExtColor {
  type Target = Color;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<ExtColor> for Color {
  fn from(ExtColor(color): ExtColor) -> Self {
    color
  }
}

impl From<Color> for ExtColor {
  fn from(value: Color) -> Self {
    ExtColor(value)
  }
}

impl Display for ExtColor {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let (r, g, b, a) = self.0.as_rgba8();
    write!(f, "#{:02x}{:02x}{:02x}{:02x}", r, g, b, a)
  }
}

impl FromStr for ExtColor {
  type Err = druid::piet::ColorParseError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    let color = Color::from_hex_str(s)?;

    Ok(color.into())
  }
}

impl Serialize for ExtColor {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_str(&self.to_string())
  }
}

impl<'de> Deserialize<'de> for ExtColor {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    struct ExtColorVisitor;

    impl<'de> Visitor<'de> for ExtColorVisitor {
      type Value = ExtColor;

      fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an rgb(a) color encoded as a hex string, with or without leading #")
      }

      fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
      where
        E: serde::de::Error,
      {
        v.parse().map_err(E::custom)
      }
    }

    deserializer.deserialize_str(ExtColorVisitor)
  }
}

impl Theme {
  pub const ACTION: ExtColor = unwrap("#004d66");
  pub const ACTION_TEXT: ExtColor = unwrap("#bbe9ff");
  pub const SUCCESS: ExtColor = unwrap("#135200");
  pub const SUCCESS_TEXT: ExtColor = unwrap("#adf68a");
  pub const ERROR: ExtColor = unwrap("#930006");
  pub const ERROR_TEXT: ExtColor = unwrap("#ffdad4");
  pub const WARNING: ExtColor = unwrap("#574500");
  pub const WARNING_TEXT: ExtColor = unwrap("#ffe174");
  pub const DO_NOT_IGNORE: ExtColor = unwrap("#7f2c00");
  pub const DO_NOT_IGNORE_TEXT: ExtColor = unwrap("#ffdbcc");

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
    env.set(util::BLUE_KEY, action.unwrap_or(Self::ACTION));
    env.set(util::ON_BLUE_KEY, action_text.unwrap_or(Self::ACTION_TEXT));
    env.set(util::GREEN_KEY, success.unwrap_or(Self::SUCCESS));
    env.set(
      util::ON_GREEN_KEY,
      success_text.unwrap_or(Self::SUCCESS_TEXT),
    );
    env.set(util::RED_KEY, error.unwrap_or(Self::ERROR));
    env.set(util::ON_RED_KEY, error_text.unwrap_or(Self::ERROR_TEXT));
    env.set(util::YELLOW_KEY, warning.unwrap_or(Self::WARNING));
    env.set(
      util::ON_YELLOW_KEY,
      warning_text.unwrap_or(Self::WARNING_TEXT),
    );
    env.set(
      util::ORANGE_KEY,
      do_not_ignore.unwrap_or(Self::DO_NOT_IGNORE),
    );
    env.set(
      util::ON_ORANGE_KEY,
      do_not_ignore_text.unwrap_or(Self::DO_NOT_IGNORE_TEXT),
    );
    if let Some(shadow) = shadow {
      env.set(SHADOW, shadow)
    }

    let mut font = env.get(druid::theme::UI_FONT);
    font.weight = FontWeight::new(450);
    env.set(druid::theme::UI_FONT, font);
  }

  pub fn random() -> Self {
    let color_faker = fake::faker::color::en::HexColor();
    let gen = || ExtColor::from(Color::from_hex_str(&color_faker.fake::<String>()).unwrap());

    Self {
      text: Some(gen()),
      button_dark: Some(gen()),
      button_light: Some(gen()),
      background_dark: gen(),
      background_light: gen(),
      border_dark: gen(),
      border_light: gen(),
      action: Some(gen()),
      action_text: Some(gen()),
      success: Some(gen()),
      success_text: Some(gen()),
      error: Some(gen()),
      error_text: Some(gen()),
      warning: Some(gen()),
      warning_text: Some(gen()),
      do_not_ignore: Some(gen()),
      do_not_ignore_text: Some(gen()),
      shadow: Some(gen()),
    }
  }
}

impl Default for Theme {
  fn default() -> Self {
    RETRO
  }
}

const fn unwrap(color: &'static str) -> ExtColor {
  match Color::from_hex_str(color) {
    Ok(color) => ExtColor(color),
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
  WardTwo,

  Custom,
}

impl From<Themes> for Theme {
  fn from(theme: Themes) -> Self {
    match theme {
      Themes::Retro => RETRO,
      Themes::Legacy => LEGACY,
      Themes::Troutman => TROUTMANST_BROOKLYN,
      Themes::Gates => GATESAVE_BROOKLYN,
      Themes::WardTwo => WARDTWO_MA,

      Themes::Custom => unimplemented!(),
    }
  }
}

// https://www.dayroselane.com/hydrants/details/40_704692_-73_924656
const TROUTMANST_BROOKLYN: Theme = Theme {
  text: Some(unwrap("#36717e")),
  button_dark: Some(unwrap("#3b0e14")),
  button_light: Some(unwrap("#3b0e14")),
  background_dark: unwrap("#050405"),
  background_light: unwrap("#1f1c1d"),
  border_dark: unwrap("#4a6d92"),
  border_light: unwrap("#5c7c8c"),
  action: None,
  action_text: None,
  success: None,
  success_text: None,
  error: None,
  error_text: None,
  warning: None,
  warning_text: None,
  do_not_ignore: None,
  do_not_ignore_text: None,
  shadow: Some(unwrap("#d0c5a7")),
};

// https://www.dayroselane.com/hydrants/details/40_699769_-73_912078
const GATESAVE_BROOKLYN: Theme = Theme {
  text: Some(unwrap("#c5ac76")),
  button_dark: Some(unwrap("#4d6263")),
  button_light: Some(unwrap("#4d6263")),
  background_dark: unwrap("#2e3939"),
  background_light: unwrap("#472a2a"),
  border_dark: unwrap("#060708"),
  border_light: unwrap("#472a2a"),
  action: None,
  action_text: None,
  success: None,
  success_text: None,
  error: None,
  error_text: None,
  warning: None,
  warning_text: None,
  do_not_ignore: None,
  do_not_ignore_text: None,
  shadow: Some(unwrap("#d0c5a7")),
};

// https://www.dayroselane.com/hydrants/details/42_377487_-71_101188
const WARDTWO_MA: Theme = Theme {
  text: Some(unwrap("#ca9582")),
  button_dark: Some(unwrap("#343836")),
  button_light: Some(unwrap("#343836")),
  background_dark: unwrap("#350f0f"),
  background_light: unwrap("#443530"),
  border_dark: unwrap("#b19f99"),
  border_light: unwrap("#9a5d41"),
  action: None,
  action_text: None,
  success: None,
  success_text: None,
  error: None,
  error_text: None,
  warning: None,
  warning_text: None,
  do_not_ignore: None,
  do_not_ignore_text: None,
  shadow: Some(unwrap("#b08584")),
};

const LEGACY: Theme = Theme {
  text: None,
  button_dark: None,
  button_light: None,
  background_dark: unwrap("#1f1a1b"),
  background_light: unwrap("#292425"),
  border_dark: unwrap("#48454f"),
  border_light: unwrap("#c9c4cf"),
  action: None,
  action_text: None,
  success: None,
  success_text: None,
  error: None,
  error_text: None,
  warning: None,
  warning_text: None,
  do_not_ignore: None,
  do_not_ignore_text: None,
  shadow: None,
};

const RETRO: Theme = Theme {
  text: Some(unwrap("#101422")),
  button_dark: Some(unwrap("#e6e6e6")),
  button_light: Some(unwrap("#e6e6e6")),
  background_dark: unwrap("#ccc3a4"),
  background_light: unwrap("#efebdd"),
  border_dark: unwrap("#101422"),
  border_light: unwrap("#161a28"),
  action: None,
  action_text: None,
  success: None,
  success_text: None,
  error: None,
  error_text: None,
  warning: None,
  warning_text: None,
  do_not_ignore: None,
  do_not_ignore_text: None,
  shadow: None,
};

pub const OLD_TEXT_COLOR: Key<Color> = Key::new("original.TEXT_COLOR");
pub const OLD_BUTTON_DARK: Key<Color> = Key::new("original.BUTTON_DARK");
pub const OLD_BUTTON_LIGHT: Key<Color> = Key::new("original.BUTTON_LIGHT");
pub const OLD_BACKGROUND_DARK: Key<Color> = Key::new("original.BACKGROUND_DARK");
pub const OLD_BACKGROUND_LIGHT: Key<Color> = Key::new("original.BACKGROUND_LIGHT");

pub fn save_original_env(env: &mut Env) {
  env.set(OLD_TEXT_COLOR, env.get(theme::TEXT_COLOR));
  env.set(OLD_BUTTON_DARK, env.get(theme::BUTTON_DARK));
  env.set(OLD_BUTTON_LIGHT, env.get(theme::BUTTON_LIGHT));
  env.set(OLD_BACKGROUND_DARK, env.get(theme::BACKGROUND_DARK));
  env.set(OLD_BACKGROUND_LIGHT, env.get(theme::BACKGROUND_LIGHT));
}

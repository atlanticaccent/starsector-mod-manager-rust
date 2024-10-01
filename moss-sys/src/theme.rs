use std::{fmt::Display, ops::Deref, str::FromStr};

use druid::{text::FontWeight, theme, Color, Data, Env, Key, Lens};
use druid_widget_nursery::prism::Prism;
use fake::Fake;
use serde::{de::Visitor, Deserialize, Serialize};

pub const ORANGE_KEY: Key<Color> = Key::new("theme.colour.orange");
pub const BLUE_KEY: Key<Color> = Key::new("theme.colour.blue");
pub const GREEN_KEY: Key<Color> = Key::new("theme.colour.green");
pub const RED_KEY: Key<Color> = Key::new("theme.colour.red");
pub const YELLOW_KEY: Key<Color> = Key::new("theme.colour.yellow");
pub const ON_GREEN_KEY: Key<Color> = Key::new("theme.colour.on_green");
pub const ON_RED_KEY: Key<Color> = Key::new("theme.colour.on_red");
pub const ON_YELLOW_KEY: Key<Color> = Key::new("theme.colour.on_yellow");
pub const ON_BLUE_KEY: Key<Color> = Key::new("theme.colour.on_blue");
pub const ON_ORANGE_KEY: Key<Color> = Key::new("theme.colour.on_orange");

pub const SHADOW: Key<Color> = Key::new("custom_theme.shadow");

#[derive(Debug, Data, Lens, Clone, Serialize, Deserialize)]
pub struct Theme {
  text: Option<ExtColor>,
  background_dark: ExtColor,
  background_light: ExtColor,
  border_dark: ExtColor,
  border_light: ExtColor,
  button_dark: Option<ExtColor>,
  button_light: Option<ExtColor>,
  shadow: Option<ExtColor>,
  action: Option<ExtColor>,
  action_text: Option<ExtColor>,
  success: Option<ExtColor>,
  success_text: Option<ExtColor>,
  warning: Option<ExtColor>,
  warning_text: Option<ExtColor>,
  do_not_ignore: Option<ExtColor>,
  do_not_ignore_text: Option<ExtColor>,
  error: Option<ExtColor>,
  error_text: Option<ExtColor>,
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
    write!(f, "#{r:02x}{g:02x}{b:02x}{a:02x}")
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
  pub const ERROR_TEXT: ExtColor = unwrap("#ff9a7e");
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
      env.set(theme::TEXT_COLOR, text);
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
    env.set(BLUE_KEY, action.unwrap_or(Self::ACTION));
    env.set(ON_BLUE_KEY, action_text.unwrap_or(Self::ACTION_TEXT));
    env.set(GREEN_KEY, success.unwrap_or(Self::SUCCESS));
    env.set(ON_GREEN_KEY, success_text.unwrap_or(Self::SUCCESS_TEXT));
    env.set(RED_KEY, error.unwrap_or(Self::ERROR));
    env.set(ON_RED_KEY, error_text.unwrap_or(Self::ERROR_TEXT));
    env.set(YELLOW_KEY, warning.unwrap_or(Self::WARNING));
    env.set(ON_YELLOW_KEY, warning_text.unwrap_or(Self::WARNING_TEXT));
    env.set(ORANGE_KEY, do_not_ignore.unwrap_or(Self::DO_NOT_IGNORE));
    env.set(
      ON_ORANGE_KEY,
      do_not_ignore_text.unwrap_or(Self::DO_NOT_IGNORE_TEXT),
    );
    if let Some(shadow) = shadow {
      env.set(SHADOW, shadow);
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
  WardLight,
  WardDark,

  HeightsLight,
  #[default]
  HeightsDark,

  Gates,
  Retro,
  Legacy,

  Custom,
}

impl From<Themes> for Theme {
  fn from(theme: Themes) -> Self {
    match theme {
      Themes::WardLight => WARD_LIGHT,
      Themes::WardDark => WARD_DARK,

      Themes::HeightsLight => HEIGHTS_LIGHT,
      Themes::HeightsDark => HEIGHTS_DARK,

      Themes::Gates => GATES_AVE_BROOKLYN,
      Themes::Retro => RETRO,
      Themes::Legacy => LEGACY,

      Themes::Custom => unimplemented!(),
    }
  }
}

// https://www.dayroselane.com/hydrants/details/40_700343_-73_995416
const HEIGHTS_DARK: Theme = Theme {
  text: Some(unwrap("#fafaffff")),
  button_dark: Some(unwrap("#272a2fff")),
  button_light: Some(unwrap("#32353aff")),
  background_dark: unwrap("#0b0e12ff"),
  background_light: unwrap("#191c20ff"),
  border_dark: unwrap("#9ea3abff"),
  border_light: unwrap("#7f838bff"),
  action: Some(unwrap("#a2cfffff")),
  action_text: Some(unwrap("#00182bff")),
  success: Some(unwrap("#abd798ff")),
  success_text: Some(unwrap("#021b00ff")),
  error: Some(unwrap("#ffbab1ff")),
  error_text: Some(unwrap("#370001ff")),
  warning: Some(unwrap("#fbc074ff")),
  warning_text: Some(unwrap("#231300ff")),
  do_not_ignore: Some(unwrap("#dfbefeff")),
  do_not_ignore_text: Some(unwrap("#22063dff")),
  shadow: Some(unwrap("#e0e2e8ff")),
};

const HEIGHTS_LIGHT: Theme = Theme {
  text: Some(unwrap("#191C20")),
  button_dark: Some(unwrap("#ECEEF4")),
  button_light: Some(unwrap("#F2F3F9")),
  background_dark: unwrap("#E0E2E8"),
  background_light: unwrap("#E6E8EE"),
  border_dark: unwrap("#73777F"),
  border_light: unwrap("#C2C7CF"),
  action: Some(unwrap("#BEE9FF")),
  action_text: Some(unwrap("#001F2A")),
  success: Some(unwrap("#C2EFAE")),
  success_text: Some(unwrap("#032100")),
  error: Some(unwrap("#FFDAD6")),
  error_text: Some(unwrap("#410002")),
  warning: Some(unwrap("#FFDDB6")),
  warning_text: Some(unwrap("#2A1800")),
  do_not_ignore: Some(unwrap("#F0DBFF")),
  do_not_ignore_text: Some(unwrap("#280D42")),
  shadow: Some(unwrap("#2D3135")),
};

// https://www.dayroselane.com/hydrants/details/42_377487_-71_101188
const WARD_LIGHT: Theme = Theme {
  text: Some(unwrap("#171d1eff")),
  button_dark: Some(unwrap("#e9eff0ff")),
  button_light: Some(unwrap("#e3e9eaff")),
  background_dark: unwrap("#dbe4e6ff"),
  background_light: unwrap("#f5fafbff"),
  border_dark: unwrap("#bfc8caff"),
  border_light: unwrap("#6f797aff"),
  action: Some(unwrap("#006874ff")),
  action_text: Some(unwrap("#ffffffff")),
  success: Some(unwrap("#3c6838ff")),
  success_text: Some(unwrap("#ffffffff")),
  error: Some(unwrap("#ba1a1aff")),
  error_text: Some(unwrap("#ffffffff")),
  warning: Some(unwrap("#775a0bff")),
  warning_text: Some(unwrap("#ffffffff")),
  do_not_ignore: Some(unwrap("#8d4e2aff")),
  do_not_ignore_text: Some(unwrap("#ffffffff")),
  shadow: Some(unwrap("#000000ff")),
};

const WARD_DARK: Theme = Theme {
  text: Some(unwrap("#f6fcfdff")),
  button_dark: Some(unwrap("#1b2122ff")),
  button_light: Some(unwrap("#252b2cff")),
  background_dark: unwrap("#0e1415ff"),
  background_light: unwrap("#1b2122ff"),
  border_dark: unwrap("#9ba5a6ff"),
  border_light: unwrap("#7b8587ff"),
  action: Some(unwrap("#499ca9ff")),
  action_text: Some(unwrap("#000000ff")),
  success: Some(unwrap("#6e9c67ff")),
  success_text: Some(unwrap("#000000ff")),
  error: Some(unwrap("#ff5449ff")),
  error_text: Some(unwrap("#000000ff")),
  warning: Some(unwrap("#c87f56ff")),
  warning_text: Some(unwrap("#000000ff")),
  do_not_ignore: Some(unwrap("#633b48ff")),
  do_not_ignore_text: Some(unwrap("#ffb68fff")),
  shadow: Some(unwrap("#c3ccceff")),
};

// https://www.dayroselane.com/hydrants/details/40_699769_-73_912078
const GATES_AVE_BROOKLYN: Theme = Theme {
  text: Some(unwrap("#c5ac76")),
  button_dark: Some(unwrap("#4d6263")),
  button_light: Some(unwrap("#4d6263")),
  background_dark: unwrap("#2e3939"),
  background_light: unwrap("#472a2a"),
  border_dark: unwrap("#060708"),
  border_light: unwrap("#947271"),
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

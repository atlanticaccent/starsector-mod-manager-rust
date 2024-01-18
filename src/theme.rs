use druid::{piet::ColorParseError, theme, Color, Data, Env, Selector};

use crate::app::util;

pub const CHANGE_THEME: Selector<Theme> = Selector::new("app.theme.change");

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
  }
}

const fn unwrap(res: Result<Color, ColorParseError>) -> Color {
  match res {
    Ok(color) => color,
    Err(_) => panic!(),
  }
}

pub fn legacy<T: Data>(env: &mut Env, _data: &T) {
  env.set(theme::BUTTON_BORDER_RADIUS, 2.);
  env.set(theme::BUTTON_BORDER_WIDTH, 2.);
  env.set(theme::BUTTON_LIGHT, env.get(theme::BUTTON_DARK));
  env.set(
    theme::BACKGROUND_DARK,
    Color::from_hex_str("#1f1a1b").unwrap(),
  );
  env.set(
    theme::BACKGROUND_LIGHT,
    Color::from_hex_str("#292425").unwrap(),
  );
  env.set(
    theme::WINDOW_BACKGROUND_COLOR,
    env.get(theme::BACKGROUND_DARK),
  );
  env.set(theme::BORDER_DARK, Color::from_hex_str("#48454f").unwrap());
  env.set(theme::BORDER_LIGHT, Color::from_hex_str("#c9c4cf").unwrap());
  env.set(util::BLUE_KEY, Color::from_hex_str("#004d66").unwrap());
  env.set(util::ON_BLUE_KEY, Color::from_hex_str("#bbe9ff").unwrap());
  env.set(util::GREEN_KEY, Color::from_hex_str("#135200").unwrap());
  env.set(util::ON_GREEN_KEY, Color::from_hex_str("#adf68a").unwrap());
  env.set(util::RED_KEY, Color::from_hex_str("#930006").unwrap());
  env.set(util::ON_RED_KEY, Color::from_hex_str("#ffdad4").unwrap());
  env.set(util::YELLOW_KEY, Color::from_hex_str("#574500").unwrap());
  env.set(util::ON_YELLOW_KEY, Color::from_hex_str("#ffe174").unwrap());
  env.set(util::ORANGE_KEY, Color::from_hex_str("#7f2c00").unwrap());
  env.set(util::ON_ORANGE_KEY, Color::from_hex_str("#ffdbcc").unwrap());
}

pub fn light<T: Data>(env: &mut Env, _data: &T) {
  env.set(theme::BUTTON_BORDER_RADIUS, 2.);
  env.set(theme::BUTTON_BORDER_WIDTH, 2.);
  env.set(theme::TEXT_COLOR, Color::from_hex_str("#101422").unwrap());
  env.set(theme::BUTTON_DARK, Color::from_hex_str("#e6e6e6").unwrap());
  env.set(theme::BUTTON_LIGHT, env.get(theme::BUTTON_DARK));
  env.set(
    theme::BACKGROUND_DARK,
    Color::from_hex_str("#ccc3a4").unwrap(),
  );
  env.set(
    theme::BACKGROUND_LIGHT,
    Color::from_hex_str("#efebdd").unwrap(),
  );
  env.set(
    theme::WINDOW_BACKGROUND_COLOR,
    env.get(theme::BACKGROUND_DARK),
  );
  env.set(theme::BORDER_DARK, Color::from_hex_str("#101422").unwrap());
  env.set(theme::BORDER_LIGHT, Color::from_hex_str("#161a28").unwrap());
  env.set(util::BLUE_KEY, Color::from_hex_str("#004d66").unwrap());
  env.set(util::ON_BLUE_KEY, Color::from_hex_str("#bbe9ff").unwrap());
  env.set(util::GREEN_KEY, Color::from_hex_str("#135200").unwrap());
  env.set(util::ON_GREEN_KEY, Color::from_hex_str("#adf68a").unwrap());
  env.set(util::RED_KEY, Color::from_hex_str("#930006").unwrap());
  env.set(util::ON_RED_KEY, Color::from_hex_str("#ffdad4").unwrap());
  env.set(util::YELLOW_KEY, Color::from_hex_str("#574500").unwrap());
  env.set(util::ON_YELLOW_KEY, Color::from_hex_str("#ffe174").unwrap());
  env.set(util::ORANGE_KEY, Color::from_hex_str("#7f2c00").unwrap());
  env.set(util::ON_ORANGE_KEY, Color::from_hex_str("#ffdbcc").unwrap());
}

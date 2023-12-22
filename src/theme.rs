use druid::{theme, Color, Data, Env};

use crate::app::util;

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
  env.set(util::GREEN_KEY, Color::from_hex_str("#135200").unwrap());
  env.set(util::RED_KEY, Color::from_hex_str("#930006").unwrap());
  env.set(util::YELLOW_KEY, Color::from_hex_str("#574500").unwrap());
  env.set(util::ON_GREEN_KEY, Color::from_hex_str("#adf68a").unwrap());
  env.set(util::ON_RED_KEY, Color::from_hex_str("#ffdad4").unwrap());
  env.set(util::ON_YELLOW_KEY, Color::from_hex_str("#ffe174").unwrap());
  env.set(util::BLUE_KEY, Color::from_hex_str("#004d66").unwrap());
  env.set(util::ON_BLUE_KEY, Color::from_hex_str("#bbe9ff").unwrap());
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
  env.set(util::GREEN_KEY, Color::from_hex_str("#135200").unwrap());
  env.set(util::RED_KEY, Color::from_hex_str("#930006").unwrap());
  env.set(util::YELLOW_KEY, Color::from_hex_str("#574500").unwrap());
  env.set(util::ON_GREEN_KEY, Color::from_hex_str("#adf68a").unwrap());
  env.set(util::ON_RED_KEY, Color::from_hex_str("#ffdad4").unwrap());
  env.set(util::ON_YELLOW_KEY, Color::from_hex_str("#ffe174").unwrap());
  env.set(util::BLUE_KEY, Color::from_hex_str("#004d66").unwrap());
  env.set(util::ON_BLUE_KEY, Color::from_hex_str("#bbe9ff").unwrap());
  env.set(util::ORANGE_KEY, Color::from_hex_str("#7f2c00").unwrap());
  env.set(util::ON_ORANGE_KEY, Color::from_hex_str("#ffdbcc").unwrap());
}

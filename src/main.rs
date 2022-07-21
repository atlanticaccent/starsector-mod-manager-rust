#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit = "1000"]
#![feature(option_zip)]
#![feature(result_flattening)]
#![feature(async_closure)]
#![feature(btree_drain_filter)]
#![feature(array_zip)]
#![feature(result_option_inspect)]
#![feature(is_some_with)]
#![feature(bool_to_option)]
#![feature(hash_set_entry)]
#![feature(string_remove_matches)]
#![feature(once_cell)]

extern crate webview_subsystem;

use clap::Parser;
use druid::{theme, AppLauncher, Color, WindowDesc};
use tokio::runtime::Builder;
use webview_shared::PROJECT;
use webview_subsystem::init_webview;

// #[path ="patch/mod.rs"]
mod app;
mod patch;
mod webview;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
  #[clap(long)]
  webview: bool,
  url: Option<String>,
}

fn main() {
  let args = Args::parse();

  std::fs::create_dir_all(PROJECT.cache_dir()).expect("Create cache dir");
  std::fs::create_dir_all(PROJECT.data_dir()).expect("Create cache dir");

  if !args.webview {
    let main_window = WindowDesc::new(app::App::ui_builder())
      .title("MOSS | Mod Organizer for StarSector")
      .window_size((1280., 1024.));

    let runtime = Builder::new_multi_thread().enable_all().build().unwrap();

    // create the initial app state
    let initial_state = app::App::new(runtime.handle().clone());

    let _guard = runtime.enter();

    // start the application
    AppLauncher::with_window(main_window)
      .configure_env(|env, _| {
        env.set(theme::BUTTON_BORDER_RADIUS, 2.);
        env.set(theme::BUTTON_BORDER_WIDTH, 2.);
        env.set(theme::BUTTON_LIGHT, env.get(theme::BUTTON_DARK));
        env.set(
          theme::BACKGROUND_DARK,
          Color::from_hex_str("1f1a1b").unwrap(),
        );
        env.set(
          theme::BACKGROUND_LIGHT,
          Color::from_hex_str("292425").unwrap(),
        );
        env.set(
          theme::WINDOW_BACKGROUND_COLOR,
          env.get(theme::BACKGROUND_DARK),
        );
        env.set(theme::BORDER_DARK, Color::from_hex_str("48454f").unwrap());
        env.set(theme::BORDER_LIGHT, Color::from_hex_str("c9c4cf").unwrap());
        env.set(theme::BORDER_LIGHT, Color::from_hex_str("c9c4cf").unwrap());
        env.set(app::util::GREEN_KEY, Color::from_hex_str("135200").unwrap());
        env.set(app::util::RED_KEY, Color::from_hex_str("930006").unwrap());
        env.set(
          app::util::YELLOW_KEY,
          Color::from_hex_str("574500").unwrap(),
        );
        env.set(
          app::util::ON_GREEN_KEY,
          Color::from_hex_str("adf68a").unwrap(),
        );
        env.set(
          app::util::ON_RED_KEY,
          Color::from_hex_str("ffdad4").unwrap(),
        );
        env.set(
          app::util::ON_YELLOW_KEY,
          Color::from_hex_str("ffe174").unwrap(),
        );
        env.set(app::util::BLUE_KEY, Color::from_hex_str("004d66").unwrap());
        env.set(
          app::util::ON_BLUE_KEY,
          Color::from_hex_str("bbe9ff").unwrap(),
        );
        env.set(
          app::util::ORANGE_KEY,
          Color::from_hex_str("7f2c00").unwrap(),
        );
        env.set(
          app::util::ON_ORANGE_KEY,
          Color::from_hex_str("ffdbcc").unwrap(),
        );
      })
      .delegate(app::AppDelegate::default())
      .launch(initial_state)
      .expect("Failed to launch application");
  } else {
    init_webview(args.url).expect("Launch webviews");
  }
}

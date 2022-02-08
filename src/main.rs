#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit="1000"]
#![feature(option_zip)]
#![feature(result_flattening)]
#![feature(async_closure)]
#![feature(btree_drain_filter)]
#![feature(array_zip)]
#![feature(result_option_inspect)]
#![feature(is_some_with)]

use druid::{AppLauncher, WindowDesc, theme};
use tokio::runtime::Builder;

#[path ="patch/mod.rs"]
mod patch;
mod app;

fn main() {
  let main_window = WindowDesc::new(app::App::ui_builder())
    .window_size((1280., 1024.));

  let runtime = Builder::new_multi_thread()
    .enable_all()
    .build()
    .unwrap();

  // create the initial app state
  let initial_state = app::App::new(runtime.handle().clone());
  
  // start the application
  AppLauncher::with_window(main_window)
    .configure_env(|env, _| {
      env.set(theme::BUTTON_BORDER_RADIUS, 2.);
      env.set(theme::BUTTON_BORDER_WIDTH, 2.);
      env.set(theme::BUTTON_LIGHT, env.get(theme::BUTTON_DARK))
    })
    .delegate(app::AppDelegate::default())
    .launch(initial_state)
    .expect("Failed to launch application");
}

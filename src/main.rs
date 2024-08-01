#![feature(let_chains)]

use const_format::concatcp;
use druid::{AppLauncher, WindowDesc};
use moss::{
  app::{app_delegate::AppDelegate, App, AppViewExt},
  theme::save_original_env,
};
use tokio::runtime::Builder;
use webview_shared::PROJECT;

fn main() {
  std::fs::create_dir_all(PROJECT.cache_dir()).expect("Create cache dir");
  std::fs::create_dir_all(PROJECT.data_dir()).expect("Create cache dir");

  let runtime = Builder::new_multi_thread().enable_all().build().unwrap();

  let _guard = runtime.enter();

  // create the initial app state
  let mut initial_state = App::new(runtime.handle().clone());

  if let Some(install_dir) = initial_state.settings.install_dir.as_ref() {
    if !install_dir.exists() {
      initial_state.settings.install_dir = None;
    }
  }

  let main_window = WindowDesc::new(App::view().overlay().theme_wrapper().env_as_shared_data())
    .title(concatcp!(
      "MOSS | Mod Organizer for StarSector v",
      env!("CARGO_PKG_VERSION")
    ))
    .window_size((1280., 1024.));

  // start the application
  AppLauncher::with_window(main_window)
    .configure_env(configure_env)
    .delegate(AppDelegate::default())
    .launch(initial_state)
    .expect("Failed to launch application");
}

fn configure_env<T>(env: &mut druid::Env, _data: &T) {
  druid_widget_nursery::configure_env(env, _data);

  save_original_env(env)
}

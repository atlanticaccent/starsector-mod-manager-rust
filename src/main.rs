#![feature(let_chains)]

use const_format::concatcp;
use druid::{AppLauncher, WindowDesc};
use moss::app::{self, app_delegate::AppDelegate};
use tokio::runtime::Builder;
use webview_shared::PROJECT;

fn main() {
  std::fs::create_dir_all(PROJECT.cache_dir()).expect("Create cache dir");
  std::fs::create_dir_all(PROJECT.data_dir()).expect("Create cache dir");

  let runtime = Builder::new_multi_thread().enable_all().build().unwrap();

  let _guard = runtime.enter();

  // create the initial app state
  let mut initial_state = app::App::new(runtime.handle().clone());

  let mut startup_popups = Vec::new();
  if let Some(install_dir) = initial_state.settings.install_dir.as_ref() {
    match app::mod_list::ModList::parse_mod_folder(install_dir.clone()) {
      Ok(mods) => initial_state.replace_mods(mods),
      Err((mods, duplicates)) => {
        initial_state.replace_mods(mods);
        for dupes in duplicates {
          startup_popups.push(app::overlays::Popup::duplicate(dupes.into()))
        }
      }
    }
  }

  let main_window = WindowDesc::new(app::App::theme_wrapper(initial_state.settings.theme.into()))
    .title(concatcp!(
      "MOSS | Mod Organizer for StarSector v",
      env!("CARGO_PKG_VERSION")
    ))
    .window_size((1280., 1024.));

  // start the application
  AppLauncher::with_window(main_window)
    .configure_env(druid_widget_nursery::configure_env)
    .delegate(AppDelegate::default().with_popups(startup_popups))
    .launch(initial_state)
    .expect("Failed to launch application");
}

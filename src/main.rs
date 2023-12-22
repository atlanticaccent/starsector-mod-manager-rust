#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit = "1000"]
#![feature(option_zip)]
#![feature(result_flattening)]
#![feature(async_closure)]
#![feature(btree_drain_filter)]
#![feature(array_zip)]
#![feature(result_option_inspect)]
#![feature(is_some_and)]
#![feature(hash_set_entry)]
#![feature(string_remove_matches)]
#![feature(io_error_more)]
#![feature(try_blocks)]
#![feature(let_chains)]
#![feature(iterator_try_collect)]
#![feature(iter_next_chunk)]
#![feature(lazy_cell)]
#![allow(clippy::new_ret_no_self)]
#![allow(clippy::type_complexity)]

extern crate webview_subsystem;

use app::app_delegate::AppDelegate;
use const_format::concatcp;
use druid::{AppLauncher, WindowDesc};
use theme::{legacy, light};
use tokio::runtime::Builder;
use webview_shared::PROJECT;

mod app;
mod nav_bar;
#[allow(dead_code)]
mod patch;
mod theme;
#[allow(dead_code)]
mod widgets;

fn main() {
  std::fs::create_dir_all(PROJECT.cache_dir()).expect("Create cache dir");
  std::fs::create_dir_all(PROJECT.data_dir()).expect("Create cache dir");

  let main_window = WindowDesc::new(app::App::view())
    .title(concatcp!(
      "MOSS | Mod Organizer for StarSector v",
      env!("CARGO_PKG_VERSION")
    ))
    .window_size((1280., 1024.));

  let runtime = Builder::new_multi_thread().enable_all().build().unwrap();

  // create the initial app state
  let initial_state = app::App::new(runtime.handle().clone());

  let _guard = runtime.enter();

  // start the application
  AppLauncher::with_window(main_window)
    .configure_env(light)
    .delegate(AppDelegate::default())
    .launch(initial_state)
    .expect("Failed to launch application");
}

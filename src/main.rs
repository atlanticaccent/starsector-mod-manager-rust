#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit="1000"]

use iced::{Application, Settings};

mod gui;
mod style;

fn main() {
  gui::App::run(Settings::default()).expect("Start main application");
}

use iced::{Application, Settings};

mod gui;
mod archive_handler;

fn main() {
  gui::App::run(Settings::default());
}

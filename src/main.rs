use iced::{Application, Settings};

mod gui;
mod archive_handler;
mod style;

fn main() {
  gui::App::run(Settings::default());
}

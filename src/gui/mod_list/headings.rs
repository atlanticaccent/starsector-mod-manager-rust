use iced::{PaneGrid, pane_grid, Button, button, Text, Row, Element, Command, Length, VerticalAlignment};
use if_chain::if_chain;

use crate::style;
use super::ModEntryComp;

#[derive(Debug, Clone)]
pub enum HeadingsMessage {
  HeadingPressed(ModEntryComp),
  Resized(pane_grid::ResizeEvent)
}

pub struct Headings {
  headings: pane_grid::State<Content>,
  pub enabled_name_split: pane_grid::Split,
  pub name_id_split: pane_grid::Split,
  pub id_author_split: pane_grid::Split,
  pub author_mod_version_split: pane_grid::Split,
  pub mod_version_auto_update_split: pane_grid::Split,
  pub auto_update_game_version_split: pane_grid::Split,
}

impl Headings {
  pub const ENABLED_PORTION: f32 = 3.0;
  pub const REMAINING_PORTION: f32 = 40.0;
  pub const ENABLED_NAME_RATIO: f32 = Headings::ENABLED_PORTION / (Headings::ENABLED_PORTION + Headings::REMAINING_PORTION);
  pub const NAME_ID_RATIO: f32 = 3.0 / 17.0;
  pub const ID_AUTHOR_RATIO: f32 = 3.0 / 14.0;
  pub const AUTHOR_MOD_VERSION_RATIO: f32 = 3.0 / 11.0;
  pub const MOD_VERSION_AUTO_UPDATE_RATIO: f32 = 3.0 / 8.0;
  pub const AUTO_UPDATE_GAME_VERSION_RATIO: f32 = 2.5 / 5.0;

  pub fn new() -> Result<Self, ()> {
    let (mut state, enabled_pane) = pane_grid::State::new(Content::new(format!("Enable"), ModEntryComp::Enabled));

    if_chain! {
      if let Some((name_pane, enabled_name_split)) = state.split(pane_grid::Axis::Vertical, &enabled_pane, Content::new(format!("Name"), ModEntryComp::Name));
      if let Some((id_pane, name_id_split)) = state.split(pane_grid::Axis::Vertical, &name_pane, Content::new(format!("ID"), ModEntryComp::ID));
      if let Some((author_pane, id_author_split)) = state.split(pane_grid::Axis::Vertical, &id_pane, Content::new(format!("Author"), ModEntryComp::Author));
      if let Some((mod_version_pane, author_mod_version_split)) = state.split(pane_grid::Axis::Vertical, &author_pane, Content::new(format!("Mod Version"), ModEntryComp::Version));
      if let Some((auto_update_pane, mod_version_auto_update_split)) = state.split(pane_grid::Axis::Vertical, &mod_version_pane, Content::new(format!("Auto-Update Supported?"), ModEntryComp::AutoUpdateSupport));
      if let Some((_, auto_update_game_version_split)) = state.split(pane_grid::Axis::Vertical, &auto_update_pane, Content::new(format!("Starsector Version"), ModEntryComp::GameVersion));
      then {
        state.resize(&enabled_name_split, Headings::ENABLED_NAME_RATIO);
        state.resize(&name_id_split, Headings::NAME_ID_RATIO);
        state.resize(&id_author_split, Headings::ID_AUTHOR_RATIO);
        state.resize(&author_mod_version_split, Headings::AUTHOR_MOD_VERSION_RATIO);
        state.resize(&mod_version_auto_update_split, Headings::MOD_VERSION_AUTO_UPDATE_RATIO);
        state.resize(&auto_update_game_version_split, Headings::AUTO_UPDATE_GAME_VERSION_RATIO);

        Ok(Headings {
          headings: state,
          enabled_name_split,
          name_id_split,
          id_author_split,
          author_mod_version_split,
          mod_version_auto_update_split,
          auto_update_game_version_split
        })
      } else {
        Err(())
      }
    }
  }

  pub fn update(&mut self, message: HeadingsMessage) -> Command<HeadingsMessage> {
    match message {
      HeadingsMessage::HeadingPressed(_) => Command::none(),
      HeadingsMessage::Resized(pane_grid::ResizeEvent { split, ratio }) => {
        if_chain! {
          if split != self.mod_version_auto_update_split || ratio < Headings::MOD_VERSION_AUTO_UPDATE_RATIO;
          if split != self.auto_update_game_version_split || ratio > Headings::AUTO_UPDATE_GAME_VERSION_RATIO;
          if split != self.enabled_name_split;
          then {
            self.headings.resize(&split, ratio);
          }
        }

        Command::none()
      }
    }
  }

  pub fn view(&mut self) -> Element<HeadingsMessage> {
    PaneGrid::new(
      &mut self.headings,
      |_, content| {
        pane_grid::Content::new(content.view())
      }
    )
    .on_resize(10, HeadingsMessage::Resized)
    .height(iced::Length::Units(50))
    .into()
  }
}

pub struct Content {
  text: String,
  button_state: button::State,
  cmp: ModEntryComp
}

impl Content {
  fn new(text: String, cmp: ModEntryComp) -> Self {
    Content {
      text,
      button_state: button::State::new(),
      cmp
    }
  }

  fn view(&mut self) -> Element<HeadingsMessage> {
    Row::new()
      .push(
        Button::new(
          &mut self.button_state,
          Text::new(self.text.clone()).height(Length::Fill).vertical_alignment(VerticalAlignment::Bottom),
        )
        .style(style::button_none::Button)
        .on_press(HeadingsMessage::HeadingPressed(self.cmp.clone()))
        .width(Length::Fill)
        .height(Length::Fill)
      )
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  }
}

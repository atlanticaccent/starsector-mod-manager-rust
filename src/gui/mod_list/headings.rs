use iced::{PaneGrid, pane_grid, Button, button, Text, Row, Space, Element, Command, Length};
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
  pub local_version_remote_version_split: pane_grid::Split,
  pub remote_version_game_version_split: pane_grid::Split,
}

impl Headings {
  pub fn new() -> Result<Self, ()> {
    let (mut state, enabled_pane) = pane_grid::State::new(Content::new(format!("Enabled"), ModEntryComp::Enabled));

    if_chain! {
      if let Some((name_pane, enabled_name_split)) = state.split(pane_grid::Axis::Vertical, &enabled_pane, Content::new(format!("Name"), ModEntryComp::Name));
      if let Some((id_pane, name_id_split)) = state.split(pane_grid::Axis::Vertical, &name_pane, Content::new(format!("ID"), ModEntryComp::ID));
      if let Some((author_pane, id_author_split)) = state.split(pane_grid::Axis::Vertical, &id_pane, Content::new(format!("Author"), ModEntryComp::Author));
      if let Some((mod_version_pane, author_mod_version_split)) = state.split(pane_grid::Axis::Vertical, &author_pane, Content::new(format!("Local Version"), ModEntryComp::Version));
      if let Some((local_version_pane, local_version_remote_version_split)) = state.split(pane_grid::Axis::Vertical, &mod_version_pane, Content::new(format!("Available Version"), ModEntryComp::GameVersion));
      if let Some((_, remote_version_game_version_split)) = state.split(pane_grid::Axis::Vertical, &local_version_pane, Content::new(format!("Starsector Version"), ModEntryComp::GameVersion));
      then {
        state.resize(&enabled_name_split, 3.0 / 43.0);
        state.resize(&name_id_split, 1.0 / 6.0);
        state.resize(&id_author_split, 1.0 / 5.0);
        state.resize(&author_mod_version_split, 1.0 / 4.0);
        state.resize(&local_version_remote_version_split, 1.0 / 3.0);
        state.resize(&remote_version_game_version_split, 1.0 / 2.0);

        Ok(Headings {
          headings: state,
          enabled_name_split,
          name_id_split,
          id_author_split,
          author_mod_version_split,
          local_version_remote_version_split,
          remote_version_game_version_split
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
        if split != self.enabled_name_split {
          self.headings.resize(&split, ratio);
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
    .height(iced::Length::Units(30))
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
      .push(Space::with_width(Length::Units(5)))
      .push(
        Button::new(
          &mut self.button_state,
          Text::new(self.text.clone())
        )
        .style(style::button_none::Button)
        .on_press(HeadingsMessage::HeadingPressed(self.cmp.clone()))
        .width(Length::Fill)
        .height(Length::Fill)
      )
      .push(Space::with_width(Length::Units(5)))
      .width(Length::Fill)
      .height(Length::Fill)
      .into()
  }
}

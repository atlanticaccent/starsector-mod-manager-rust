use iced::{PaneGrid, pane_grid, Button, button, Text, Element};

#[derive(Debug, Clone)]
pub enum HeadingsMessage {
  HeadingPressed(String)
}

pub struct Headings {
  headings: pane_grid::State<Content>
}

impl Headings {
  pub fn new() {
    let (mut state, pane) = pane_grid::State::new(Content::new(format!("Enabled")));

    if let Some((pane, split)) = state.split(pane_grid::Axis::Vertical, &pane, Content::new(format!("Name"))) {
      state.resize(&split, 3.0 / 13.0);
    };
  }
}

pub struct Content {
  text: String,
  button_state: button::State
}

impl Content {
  fn new(text: String) -> Self {
    Content {
      text,
      button_state: button::State::new()
    }
  }

  fn view(&mut self) -> Element<HeadingsMessage> {
    Button::new(
      &mut self.button_state,
      Text::new(self.text.clone())
    )
    .on_press(HeadingsMessage::HeadingPressed(self.text.clone()))
    .into()
  }
}

// Row::new()
//   .push(
//     Button::new(
//       &mut self.enabled_sort_state,
//       Text::new("Enabled")
//     )
//     .padding(0)
//     .width(Length::FillPortion(3))
//     .style(style::button_none::Button)
//     .on_press(ModListMessage::SetSorting(ModEntryComp::Enabled))
//   )
//   .push(
//     Row::new()
//       .push(Space::with_width(Length::Units(1)))
//       .push(
//         Button::new(
//           &mut self.name_sort_state,
//           Text::new("Name")
//         )
//         .padding(0)
//         .width(Length::Fill)
//         .style(style::button_none::Button)
//         .on_press(ModListMessage::SetSorting(ModEntryComp::Name))
//       )
//       // .push(Rule::vertical(0).style(style::max_rule::Rule))
//       .push(Space::with_width(Length::Units(6)))
//       .push(
//         Button::new(
//           &mut self.id_sort_state,
//           Text::new("ID")
//         )
//         .padding(0)
//         .width(Length::Fill)
//         .style(style::button_none::Button)
//         .on_press(ModListMessage::SetSorting(ModEntryComp::ID))
//       )
//       // .push(Rule::vertical(0).style(style::max_rule::Rule))
//       .push(Space::with_width(Length::Units(6)))
//       .push(
//         Button::new(
//           &mut self.author_sort_state,
//           Text::new("Author")
//         )
//         .padding(0)
//         .width(Length::Fill)
//         .style(style::button_none::Button)
//         .on_press(ModListMessage::SetSorting(ModEntryComp::Author))
//       )
//       // .push(Rule::vertical(0).style(style::max_rule::Rule))
//       .push(Space::with_width(Length::Units(6)))
//       .push(
//         Button::new(
//           &mut self.version_sort_state,
//           Text::new("Mod Version")
//         )
//         .padding(0)
//         .width(Length::Fill)
//         .style(style::button_none::Button)
//         .on_press(ModListMessage::SetSorting(ModEntryComp::Version))
//       )
//       // .push(Rule::vertical(0).style(style::max_rule::Rule))
//       .push(Space::with_width(Length::Units(6)))
//       .push(
//         Button::new(
//           &mut self.game_version_sort_state,
//           Text::new("Starsector Version")
//         )
//         .padding(0)
//         .width(Length::Fill)
//         .style(style::button_none::Button)
//         .on_press(ModListMessage::SetSorting(ModEntryComp::GameVersion))
//       )
//       .width(Length::FillPortion(40))
//   )
//   .push(Space::with_width(Length::Units(10)))
//   .height(Length::Shrink)
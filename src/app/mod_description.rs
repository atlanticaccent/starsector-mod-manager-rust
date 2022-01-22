use std::sync::Arc;

use druid::{Widget, widget::{Label, Flex, Scroll, Button}, WidgetExt, LensExt};

use super::ModEntry;

use super::util::{LabelExt, make_description_row};

#[derive(Default)]
pub struct ModDescription {}

impl ModDescription {
  pub fn ui_builder() -> impl Widget<Arc<ModEntry>> {
    Flex::column()
      .with_flex_child(
        Flex::row()
          .with_flex_child(
            Flex::column()
              .with_child(make_description_row(
                Label::wrapped("Name:"),
                Label::wrapped_lens(ModEntry::name.in_arc())
              )).with_child(make_description_row(
                Label::wrapped("ID:"),
                Label::wrapped_lens(ModEntry::id.in_arc())
              )).with_child(make_description_row(
                Label::wrapped("Author(s):"),
                Label::wrapped_lens(ModEntry::author.in_arc())
              )).with_child(make_description_row(
                Label::wrapped("Enabled:"),
                Label::wrapped_lens(ModEntry::enabled.in_arc().map(|e| e.to_string(), |_, _| {}))
              )).with_child(make_description_row(
                Label::wrapped("Version:"),
                Label::wrapped_lens(ModEntry::version.in_arc().map(|v| v.to_string(), |_, _| {}))
              )).expand(),
            1.
          ).with_flex_child(
            Flex::column()
              .with_child(Label::new("Description:").with_text_alignment(druid::TextAlignment::Start).expand_width())
              .with_flex_child(
            Scroll::new(Label::dynamic(|t: &String, _| t.to_string()).with_line_break_mode(druid::widget::LineBreaking::WordWrap).lens(ModEntry::description.in_arc())).vertical().expand(),
                1.
              ).expand(),
            1.
          ).expand_height(),
        1.
      ).with_child(
        Button::new("Open in file manager...").align_right().expand_width()
      ).padding(5.)
  }

  pub fn empty_builder() -> impl Widget<()> {
    Label::new("No mod selected.")
  }
}

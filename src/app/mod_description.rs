use std::sync::Arc;

use chrono::{DateTime, Local};
use druid::{
  widget::{Button, Flex, Label, Maybe, Scroll},
  LensExt, Widget, WidgetExt, Selector,
};

use super::{mod_entry::{ModVersionMeta, ModMetadata}, ModEntry};

use super::util::{make_flex_description_row, LabelExt};

pub const OPEN_IN_BROWSER: Selector<String> = Selector::new("mod_description.forum.open_in_webview");

#[derive(Default)]
pub struct ModDescription;

impl ModDescription {
  pub const FRACTAL_URL: &'static str = "https://fractalsoftworks.com/forum/index.php?topic=";
  pub const NEXUS_URL: &'static str = "https://www.nexusmods.com/starsector/mods/";

  pub fn ui_builder() -> impl Widget<Arc<ModEntry>> {
    Flex::column()
      .with_flex_child(
        Flex::row()
          .with_flex_child(
            Flex::column()
              .with_child(make_flex_description_row(
                Label::wrapped("Name:"),
                Label::wrapped_lens(ModEntry::name.in_arc()),
              ))
              .with_child(make_flex_description_row(
                Label::wrapped("ID:"),
                Label::wrapped_lens(ModEntry::id.in_arc()),
              ))
              .with_child(make_flex_description_row(
                Label::wrapped("Author(s):"),
                Label::wrapped_lens(ModEntry::author.in_arc()),
              ))
              .with_child(make_flex_description_row(
                Label::wrapped("Enabled:"),
                Label::wrapped_lens(ModEntry::enabled.in_arc().map(|e| e.to_string(), |_, _| {})),
              ))
              .with_child(make_flex_description_row(
                Label::wrapped("Version:"),
                Label::wrapped_lens(ModEntry::version.in_arc().map(|v| v.to_string(), |_, _| {})),
              ))
              .with_child(
                make_flex_description_row(
                  Label::wrapped("Installed at:"),
                  Label::wrapped_func(|data: &ModMetadata, _| if let Some(date) = data.install_date {
                    DateTime::<Local>::from(date).format("%v %I:%M%p").to_string()
                  } else {
                    String::from("Unknown")
                  })
                ).lens(ModEntry::manager_metadata.in_arc())
              )
              .with_child(
                Maybe::or_empty(|| {
                  Maybe::or_empty(|| {
                    make_flex_description_row(
                      Label::wrapped("Fractal link:"),
                      Button::from_label(Label::wrapped_func(|data: &String, _: &druid::Env| {
                        format!("{}{}", ModDescription::FRACTAL_URL, data.clone())
                      }))
                      .on_click(|ctx, data, _| {
                        ctx.submit_command(OPEN_IN_BROWSER.with(format!("{}{}", ModDescription::FRACTAL_URL, data)))
                      }),
                    )
                  })
                  .lens(ModVersionMeta::fractal_id.map(
                    |id| {
                      if !id.is_empty() {
                        Some(id.clone())
                      } else {
                        None
                      }
                    },
                    |_, _| {},
                  ))
                })
                .lens(ModEntry::version_checker.in_arc()),
              )
              .with_child(
                Maybe::or_empty(|| {
                  Maybe::or_empty(|| {
                    make_flex_description_row(
                      Label::wrapped("Nexus link:"),
                      Button::from_label(Label::wrapped_func(|data: &String, _: &druid::Env| {
                        format!("{}{}", ModDescription::NEXUS_URL, data.clone())
                      }))
                      .on_click(|ctx, data, _| {
                        ctx.submit_command(OPEN_IN_BROWSER.with(format!("{}{}", ModDescription::NEXUS_URL, data)))
                      }),
                    )
                  })
                  .lens(ModVersionMeta::nexus_id.map(
                    |id| {
                      if !id.is_empty() {
                        Some(id.clone())
                      } else {
                        None
                      }
                    },
                    |_, _| {},
                  ))
                })
                .lens(ModEntry::version_checker.in_arc()),
              )
              .expand(),
            1.,
          )
          .with_flex_child(
            Flex::column()
              .with_child(
                Label::new("Description:")
                  .with_text_alignment(druid::TextAlignment::Start)
                  .expand_width(),
              )
              .with_flex_child(
                Scroll::new(
                  Label::dynamic(|t: &String, _| t.to_string())
                    .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
                    .lens(ModEntry::description.in_arc()),
                )
                .vertical()
                .expand(),
                1.,
              )
              .expand(),
            1.,
          )
          .expand_height(),
        1.,
      )
      .with_child(
        Button::new("Open in file manager...")
          .on_click(|_, data: &mut Arc<ModEntry>, _| {
            if let Err(err) = opener::open(data.path.clone()) {
              eprintln!("{}", err)
            }
          })
          .align_right()
          .expand_width(),
      )
      .padding(5.)
  }

  pub fn empty_builder() -> impl Widget<()> {
    Label::new("No mod selected.")
  }
}

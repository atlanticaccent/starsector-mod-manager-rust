use std::sync::Arc;

use chrono::{DateTime, Local};
use druid::{
  lens::{Constant, Identity, InArc},
  widget::{Button, Flex, Label, ZStack},
  Key, LensExt, Selector, UnitPoint, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, prism::OptionSome};

use crate::{
  nav_bar::{Nav, NavLabel},
  widgets::card::Card,
};

use super::{
  mod_entry::{ModMetadata, ModVersionMeta},
  util::{
    h1, h2_fixed, h3, h3_fixed, lensed_bold, Compute, LabelExt,
    LensExtExt, WidgetExtEx, CHEVRON_LEFT, GREEN_KEY, ON_GREEN_KEY, ON_RED_KEY, RED_KEY,
  },
  ModEntry,
};

pub const OPEN_IN_BROWSER: Selector<String> =
  Selector::new("mod_description.forum.open_in_webview");

#[derive(Default)]
pub struct ModDescription;

impl ModDescription {
  pub const FRACTAL_URL: &'static str = "https://fractalsoftworks.com/forum/index.php?topic=";
  pub const NEXUS_URL: &'static str = "https://www.nexusmods.com/starsector/mods/";

  pub fn view() -> impl Widget<Arc<ModEntry>> {
    let title_text = || {
      lensed_bold(
        druid::theme::TEXT_SIZE_NORMAL,
        druid::FontWeight::SEMI_BOLD,
        druid::theme::TEXT_COLOR,
      )
      .padding((8.0, 0.0))
    };

    Flex::column()
      .with_child(
        Flex::row()
          .with_child(
            Card::hoverable(
              || {
                ZStack::new(
                  title_text()
                    .lens(Constant("Back".to_owned()))
                    .padding((8.0, 0.0, 0.0, 0.0)),
                )
                .with_aligned_child(Icon::new(CHEVRON_LEFT), UnitPoint::LEFT)
                .align_vertical_centre()
                .expand_height()
              },
              (0.0, 14.0),
            )
            .fix_height(52.0)
            .padding((0.0, 5.0))
            .on_click(|ctx, _, _| ctx.submit_command(Nav::NAV_SELECTOR.with(NavLabel::Mods))),
          )
          .with_flex_child(
            Card::builder()
              .with_insets((0.0, 14.0))
              .with_corner_radius(4.0)
              .with_shadow_length(6.0)
              .build(
                title_text()
                  .lens(
                    ModEntry::name.then(Compute::new(|t| format!("Mods  /  {}  /  Details", t))),
                  )
                  .align_vertical_centre()
                  .align_left(),
              )
              .fix_height(52.0)
              .padding((0.0, 5.0))
              .expand_width(),
            1.0,
          )
          .with_child(
            Card::builder()
              .with_insets(14.0)
              .with_border(0.5, Key::new("enabled_card.border"))
              .hoverable(|| {
                Label::new("Enabled")
                  .else_if(|data, _| !data, Label::new("Disabled"))
                  .align_vertical_centre()
                  .expand_height()
              })
              .env_scope(|env, data| {
                if *data {
                  env.set(druid::theme::BACKGROUND_LIGHT, env.get(GREEN_KEY));
                  env.set(druid::theme::TEXT_COLOR, env.get(ON_GREEN_KEY));
                  env.set(
                    Key::<druid::Color>::new("enabled_card.border"),
                    env.get(ON_GREEN_KEY),
                  );
                } else {
                  env.set(druid::theme::BACKGROUND_LIGHT, env.get(RED_KEY));
                  env.set(druid::theme::TEXT_COLOR, env.get(ON_RED_KEY));
                  env.set(
                    Key::<druid::Color>::new("enabled_card.border"),
                    env.get(ON_RED_KEY),
                  );
                }
              })
              .fix_size(100.0, 52.0)
              .padding((0.0, 5.0))
              .on_click(|_, data, _| *data = !*data)
              .lens(ModEntry::enabled),
          )
          .expand_width(),
      )
      .with_flex_child(
        Card::builder()
          .with_corner_radius(4.0)
          .with_shadow_length(6.0)
          .with_insets((0.0, 14.0))
          .build(
            Flex::column()
              .with_child(
                Flex::row()
                  .with_child(h1().lens(ModEntry::name))
                  .with_child(
                    Flex::row()
                      .with_spacer(5.0)
                      .with_child(h3_fixed("id: "))
                      .with_child(h3().lens(ModEntry::id))
                      .padding((0.0, 4.5, 0.0, 0.0)),
                  ),
              )
              .with_child(h2_fixed("Version:"))
              .with_child(Label::wrapped_lens(
                ModEntry::version.compute(|t| t.to_string()),
              ))
              .with_child(h2_fixed("Author(s):"))
              .with_child(Label::wrapped_lens(ModEntry::author))
              .with_child(h2_fixed("Installed at:"))
              .with_child(
                Label::wrapped_func(|data: &ModMetadata, _| {
                  if let Some(date) = data.install_date {
                    DateTime::<Local>::from(date)
                      .format("%v %I:%M%p")
                      .to_string()
                  } else {
                    String::from("Unknown")
                  }
                })
                .lens(ModEntry::manager_metadata),
              )
              .with_child(
                h2_fixed("Forum thread:")
                  .prism(OptionSome)
                  .lens(ModVersionMeta::fractal_id.compute(|s| (!s.is_empty()).then(|| s.clone())))
                  .prism(OptionSome)
                  .lens(ModEntry::version_checker),
              )
              .with_child(
                Button::from_label(Label::wrapped_func(|data: &String, _: &druid::Env| {
                  format!("{}{}", ModDescription::FRACTAL_URL, data.clone())
                }))
                .on_click(|ctx, data, _| {
                  ctx.submit_command(OPEN_IN_BROWSER.with(format!(
                    "{}{}",
                    ModDescription::FRACTAL_URL,
                    data
                  )))
                })
                .prism(OptionSome)
                .lens(ModVersionMeta::fractal_id.compute(|s| (!s.is_empty()).then(|| s.clone())))
                .prism(OptionSome)
                .lens(ModEntry::version_checker),
              )
              .with_child(
                h2_fixed("NexusMods page:")
                  .prism(OptionSome)
                  .lens(ModVersionMeta::nexus_id.compute(|s| (!s.is_empty()).then(|| s.clone())))
                  .prism(OptionSome)
                  .lens(ModEntry::version_checker),
              )
              .with_child(
                Button::from_label(Label::wrapped_func(|data: &String, _: &druid::Env| {
                  format!("{}{}", ModDescription::NEXUS_URL, data.clone())
                }))
                .on_click(|ctx, data, _| {
                  ctx.submit_command(OPEN_IN_BROWSER.with(format!(
                    "{}{}",
                    ModDescription::NEXUS_URL,
                    data
                  )))
                })
                .prism(OptionSome)
                .lens(ModVersionMeta::nexus_id.compute(|s| (!s.is_empty()).then(|| s.clone())))
                .prism(OptionSome)
                .lens(ModEntry::version_checker),
              )
              .with_flex_spacer(1.0)
              .with_child(
                Button::new("Open in file manager...")
                  .on_click(|_, data: &mut ModEntry, _| {
                    if let Err(err) = opener::open(data.path.clone()) {
                      eprintln!("{}", err)
                    }
                  })
                  .align_right()
                  .expand_width(),
              )
              .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
              .expand()
              .padding(5.0),
          )
          .expand_height(),
        1.0,
      )
      .lens(InArc::new::<ModEntry, ModEntry>(Identity))
      .expand_height()
  }

  pub fn empty_builder() -> impl Widget<()> {
    Label::new("No mod selected.")
  }
}

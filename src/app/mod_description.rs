use chrono::{DateTime, Local};
use druid::{
  lens::Constant,
  widget::{Flex, Label, Maybe, ZStack},
  Color, Key, LensExt, Selector, UnitPoint, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, Mask};

use crate::{
  nav_bar::{Nav, NavLabel},
  widgets::card::Card,
};

use super::{
  controllers::Rotated,
  mod_entry::{ModMetadata, UpdateStatus},
  overlays::Popup,
  util::{
    h1, h2_fixed, h3, h3_fixed, hoverable_text, lensed_bold, Compute, LabelExt, LensExtExt,
    ShadeColor, WidgetExtEx, BLUE_KEY, CHEVRON_LEFT, DELETE, GREEN_KEY, ON_BLUE_KEY, ON_GREEN_KEY,
    ON_RED_KEY, RED_KEY, SYSTEM_UPDATE, TOGGLE_ON,
  },
  ViewModEntry as ModEntry,
};

pub const OPEN_IN_BROWSER: Selector<String> =
  Selector::new("mod_description.forum.open_in_webview");

#[derive(Default)]
pub struct ModDescription;

impl ModDescription {
  pub const FRACTAL_URL: &'static str = "https://fractalsoftworks.com/forum/index.php?topic=";
  pub const NEXUS_URL: &'static str = "https://www.nexusmods.com/starsector/mods/";

  pub fn view() -> impl Widget<ModEntry> {
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
                .with_aligned_child(Icon::new(*CHEVRON_LEFT), UnitPoint::LEFT)
                .valign_centre()
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
                  .valign_centre()
                  .align_left(),
              )
              .fix_height(52.0)
              .padding((0.0, 5.0))
              .expand_width(),
            1.0,
          )
          .expand_width(),
      )
      .with_flex_child(
        Mask::new(Self::body())
          .dynamic(|data, _| data.view_state.updating)
          .with_text_mask("Updating"),
        1.0,
      )
      .with_child(
        Flex::row().with_flex_spacer(1.0).with_child(
          Card::hoverable(
            || {
              title_text()
                .lens(Constant("Open in file manager...".to_owned()))
                .valign_centre()
                .expand_height()
            },
            (0.0, 14.0),
          )
          .fix_height(52.0)
          .padding((0.0, 5.0))
          .on_click(|_, data: &mut ModEntry, _| {
            if let Err(err) = opener::open(data.path.clone()) {
              eprintln!("{}", err)
            }
          }),
        ),
      )
      .expand_height()
  }

  fn body() -> impl Widget<ModEntry> {
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
          .with_spacer(4.0)
          .with_child(
            Flex::row()
              .with_child(
                Card::builder()
                  .with_insets((0.0, 8.0))
                  .with_corner_radius(6.0)
                  .with_shadow_length(2.0)
                  .with_shadow_increase(2.0)
                  .with_border(2.0, Key::new("enabled_card.border"))
                  .hoverable(|| {
                    Flex::row()
                      .with_child(
                        Rotated::new(Icon::new(*TOGGLE_ON), 3)
                          .else_if(|data, _| !data, Rotated::new(Icon::new(*TOGGLE_ON), 1))
                          .padding((5.0, 0.0, -5.0, 0.0)),
                      )
                      .with_child(
                        Label::new("Enabled")
                          .else_if(|data, _| !data, Label::new("Disabled"))
                          .align_horizontal(UnitPoint::CENTER)
                          .fix_width(80.0),
                      )
                      .valign_centre()
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
                  .fix_height(42.0)
                  .padding((-4.0, 2.0, 0.0, 2.0))
                  .on_click(|_, data, _| *data = !*data)
                  .lens(ModEntry::enabled),
              )
              .with_child(
                Card::builder()
                  .with_insets((0.0, 8.0))
                  .with_corner_radius(6.0)
                  .with_shadow_length(2.0)
                  .with_shadow_increase(2.0)
                  .with_border(2.0, Key::<druid::Color>::new("enabled_card.border"))
                  .hoverable(|| {
                    Flex::row()
                      .with_child(Icon::new(*SYSTEM_UPDATE).padding((5.0, 0.0, -5.0, 0.0)))
                      .with_child(Label::new("Install Latest Update").padding((10.0, 0.0)))
                      .valign_centre()
                  })
                  .env_scope(|env, _| {
                    env.set(druid::theme::BACKGROUND_LIGHT, env.get(BLUE_KEY));
                    env.set(druid::theme::TEXT_COLOR, env.get(ON_BLUE_KEY));
                    env.set(
                      Key::<druid::Color>::new("enabled_card.border"),
                      env.get(ON_BLUE_KEY),
                    );
                  })
                  .fix_height(42.0)
                  .padding((0.0, 2.0))
                  .or_empty(|data: &ModEntry, _| {
                    data
                      .remote_version
                      .as_ref()
                      .is_some_and(|r| r.direct_download_url.is_some())
                      && data.update_status.as_ref().is_some_and(|s| {
                        matches!(
                          s,
                          UpdateStatus::Major(_) | UpdateStatus::Minor(_) | UpdateStatus::Patch(_)
                        )
                      })
                  })
                  .on_click(|ctx, data, _| {
                    ctx.submit_command(Popup::OPEN_POPUP.with(Popup::remote_update(data)))
                  })
                  .disabled_if(|data, _| data.view_state.updating),
              )
              .with_child(
                Card::builder()
                  .with_insets((0.0, 8.0))
                  .with_corner_radius(6.0)
                  .with_shadow_length(2.0)
                  .with_shadow_increase(2.0)
                  .with_border(2.0, druid::Color::WHITE.darker())
                  .with_background(druid::Color::BLACK.lighter().lighter())
                  .hoverable(|| {
                    Flex::row()
                      .with_child(Icon::new(*DELETE).padding((5.0, 0.0, -5.0, 0.0)))
                      .with_child(Label::new("Uninstall").padding((10.0, 0.0)))
                      .valign_centre()
                  })
                  .env_scope(|env, _| {
                    env.set(druid::theme::TEXT_COLOR, druid::Color::WHITE.darker())
                  })
                  .fix_height(42.0)
                  .padding((0.0, 2.0))
                  .on_click(|ctx, data: &mut ModEntry, _| {
                    let data: &ModEntry = data;
                    ctx.submit_command(Popup::OPEN_POPUP.with(Popup::ConfirmDelete(data.into())))
                  }),
              ),
          )
          .with_child(h2_fixed("Version"))
          .with_child(Label::wrapped_lens(
            ModEntry::version.compute(|t| t.to_string()),
          ))
          .with_default_spacer()
          .with_child(
            Maybe::or_empty(|| {
              Flex::column()
                .with_child(h2_fixed("Newer version"))
                .with_child(Label::stringify_wrapped())
                .with_default_spacer()
                .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            })
            .lens(ModEntry::update_status.compute(|s| {
              s.clone().filter(|s| {
                matches!(
                  s,
                  UpdateStatus::Major(_)
                    | UpdateStatus::Minor(_)
                    | UpdateStatus::Patch(_)
                    | UpdateStatus::UpToDate
                )
              })
            })),
          )
          .with_child(h2_fixed("Author(s)"))
          .with_child(Label::wrapped_lens(ModEntry::author))
          .with_default_spacer()
          .with_child(h2_fixed("Installed at"))
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
          .with_default_spacer()
          .with_child(h2_fixed("Description"))
          .with_child(Label::stringify_wrapped().lens(ModEntry::description))
          .with_default_spacer()
          .with_child(
            Maybe::or_empty(|| h2_fixed("Forum thread")).lens(Compute::new(ModEntry::fractal_link)),
          )
          .with_spacer(4.0)
          .with_child(
            Maybe::or_empty(|| {
              hoverable_text(Some(Color::rgb8(0x00, 0x7B, 0xFF)))
                .on_click(|ctx, data, _| ctx.submit_command(OPEN_IN_BROWSER.with(data.clone())))
            })
            .lens(Compute::new(ModEntry::fractal_link)),
          )
          .with_default_spacer()
          .with_child(
            Maybe::or_empty(|| h2_fixed("NexusMods page")).lens(Compute::new(ModEntry::nexus_link)),
          )
          .with_spacer(4.0)
          .with_child(
            Maybe::or_empty(|| {
              hoverable_text(Some(Color::rgb8(0x00, 0x7B, 0xFF)))
                .on_click(|ctx, data, _| ctx.submit_command(OPEN_IN_BROWSER.with(data.clone())))
            })
            .lens(Compute::new(ModEntry::nexus_link)),
          )
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .must_fill_main_axis(true)
          .expand()
          .padding(5.0),
      )
      .expand_height()
  }

  pub fn empty_builder() -> impl Widget<()> {
    Label::new("No mod selected.")
  }
}

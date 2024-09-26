use std::{convert::identity, sync::Arc};

use chrono::{DateTime, Local};
use druid::{
  im::{vector, Vector},
  lens::{Constant, Map},
  widget::{Flex, Label, List, Maybe, Painter, ZStack},
  Color, Data, Key, KeyOrValue, Lens, LensExt, Selector, UnitPoint, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, Mask, WidgetExt as _};
use itertools::Itertools;

use crate::{
  app::{
    app_delegate::AppCommands,
    controllers::{HoverController, Rotated, REMOVE_POINTER},
    mod_entry::{ModMetadata, UpdateStatus, VersionComplex},
    mod_list::ModList,
    overlays::Popup,
    theme::{self, BLUE_KEY, GREEN_KEY, ON_BLUE_KEY, ON_GREEN_KEY, ON_RED_KEY, RED_KEY},
    util::{
      bolded, h1, h2_fixed, h3, h3_fixed, hoverable_text_opts, ident_arc, lensed_bold, Compute,
      FastImMap, LabelExt, LensExtExt, ShadeColor, WidgetExtEx, WithHoverState, CHEVRON_LEFT,
      DELETE, HOVER_STATE_CHANGE, SYSTEM_UPDATE, TOGGLE_ON,
    },
    App, ViewModEntry as ModEntry, INFO,
  },
  nav_bar::{Nav, NavLabel},
  widgets::card::Card,
};

pub const OPEN_IN_BROWSER: Selector<String> =
  Selector::new("mod_description.forum.open_in_webview");
pub const ENABLE_DEPENDENCIES: Selector<String> =
  Selector::new("mod_description.enabled.enable_dependencies");

#[derive(Debug, Clone, Data, Lens)]
pub struct ModDescription<T = Arc<ModEntry>> {
  entry: T,
  crumbs: Vector<(String, String)>,
}

impl ModDescription<String> {
  pub fn from_entry(entry: &ModEntry) -> Self {
    Self {
      entry: entry.id.clone(),
      crumbs: vector![(entry.name.clone(), entry.id.clone())],
    }
  }
}

impl ModDescription {
  pub const FRACTAL_URL: &'static str = "https://fractalsoftworks.com/forum/index.php?topic=";
  pub const NEXUS_URL: &'static str = "https://www.nexusmods.com/starsector/mods/";

  pub const DEP_MAP: Key<std::sync::Arc<FastImMap<String, UpdateStatus>>> =
    Key::new("mod_description.dep_map");
  const DEP_TEXT_COLOR: Key<Color> = Key::new("mod_description.dependencies.link_colour");
  const DEP_TEXT_BG_COLOR: Key<Color> = Key::new("mod_description.dependencies.link_bg_colour");

  const NOTIF_OPEN_DEP: Selector<String> = Selector::new("mod_description.dependencies.open");

  pub fn from_entry_self(entry: Arc<ModEntry>, other: ModDescription<String>) -> Self {
    Self {
      entry,
      crumbs: other.crumbs,
    }
  }

  pub fn wrapped_view() -> impl Widget<App> {
    Maybe::new(|| ModDescription::view(), ModDescription::empty_builder)
      .lens(Map::new(
        |app: &App| {
          app.active.as_ref().and_then(|desc| {
            app
              .mod_list
              .mods
              .get(&desc.entry)
              .cloned()
              .map(|entry| ModDescription::from_entry_self(entry, desc.clone()))
          })
        },
        |app, entry| {
          if let Some(desc) = entry {
            app.mod_list.mods.insert(desc.entry.id.clone(), desc.entry);
          }
        },
      ))
      .env_scope(|env, data| {
        if let Some(entry) = data
          .active
          .as_ref()
          .and_then(|desc| data.mod_list.mods.get(&desc.entry))
        {
          let found_deps = entry
            .dependencies
            .iter()
            .filter_map(|dep| {
              dep.version.as_ref().map(|version| {
                let found_version = data.mod_list.mods.get(&dep.id).map(|found| &found.version);
                let status = match (version, found_version) {
                  (version, Some(found_version)) if version == found_version => {
                    UpdateStatus::UpToDate
                  }
                  (version, Some(found_version)) if version.major() == found_version.major() => {
                    UpdateStatus::Minor(VersionComplex::DUMMY)
                  }
                  _ => UpdateStatus::Error,
                };
                (dep.id.clone(), status)
              })
            })
            .collect();

          env.set(ModDescription::DEP_MAP, std::sync::Arc::new(found_deps));
        }
      })
      .on_notification(ModDescription::NOTIF_OPEN_DEP, |ctx, id, app| {
        if let Some(notif_entry) = app.mod_list.mods.get(id) {
          let mut crumbs = if let Some(current_desc) = app.active.take() {
            current_desc.crumbs
          } else {
            Vector::new()
          };
          crumbs.push_back((notif_entry.name.clone(), notif_entry.id.clone()));
          app.active = Some(ModDescription {
            entry: notif_entry.id.clone(),
            crumbs,
          })
        }
        ctx.submit_command(HOVER_STATE_CHANGE);
        ctx.submit_command(REMOVE_POINTER);
        ctx.set_handled();
      })
  }

  pub fn view() -> impl Widget<ModDescription> {
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
            .on_click(|ctx, desc: &mut ModDescription, _| {
              if desc.crumbs.len() > 1 {
                desc.crumbs.pop_back();
                ctx.submit_command(App::SELECTOR.with(AppCommands::UpdateModDescription(
                  ModDescription {
                    entry: desc.crumbs.last().unwrap().1.clone(),
                    crumbs: desc.crumbs.split_off(0),
                  },
                )))
              } else {
                ctx.submit_command(Nav::NAV_SELECTOR.with(NavLabel::Mods))
              }
            }),
          )
          .with_flex_child(
            Card::builder()
              .with_insets((0.0, 14.0))
              .with_corner_radius(4.0)
              .with_shadow_length(6.0)
              .build(
                title_text()
                  .lens(ModDescription::crumbs.then(Compute::new(
                    |crumbs: &Vector<(String, String)>| {
                      format!(
                        "Mods  /  {}  /  Details",
                        crumbs.iter().map(|(name, _)| name).join("  /  ")
                      )
                    },
                  )))
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
          .with_text_mask("Updating")
          .lens(ModDescription::entry.then(ident_arc::<ModEntry>())),
        1.0,
      )
      .with_child(
        Flex::row()
          .with_flex_spacer(1.0)
          .with_child(
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
          )
          .lens(ModDescription::entry.then(ident_arc::<ModEntry>())),
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
                  .with_border(3.5, Key::new("enabled_card.border"))
                  .hoverable(|_| {
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
                  .lens(ModEntry::enabled)
                  .on_change(notify_enabled),
              )
              .with_child(
                Card::builder()
                  .with_insets((0.0, 8.0))
                  .with_corner_radius(6.0)
                  .with_shadow_length(2.0)
                  .with_shadow_increase(2.0)
                  .with_border(3.5, Key::<druid::Color>::new("enabled_card.border"))
                  .hoverable(|_| {
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
                  .empty_if_not(|data: &ModEntry, _| {
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
                  .with_border(3.5, druid::Color::WHITE.darker())
                  .with_background(druid::Color::BLACK.lighter().lighter())
                  .hoverable(|_| {
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
          .with_child(
            Maybe::or_empty(|| {
              Flex::column()
                .with_child(h2_fixed("Author(s)"))
                .with_child(Label::wrapped_func(|data: &String, _| data.clone()))
                .with_default_spacer()
                .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            })
            .lens(ModEntry::author),
          )
          .with_child(
            Flex::column()
              .with_child(
                Flex::row()
                  .with_child(h2_fixed("Utility"))
                  .with_spacer(5.0)
                  .with_child(
                    Icon::new(*INFO)
                      .fix_size(15.0, 15.0)
                      .with_hover_state(false)
                      .stack_tooltip_custom(Card::new(
                        Flex::column()
                          .with_child(bolded(
                            "Utility mods should always be possible to uninstall without breaking",
                          ))
                          .with_child(bolded("saves."))
                          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
                          .padding((7.0, 0.0)),
                      )),
                  ),
              )
              .with_child(Label::wrapped_func(|data: &bool, _| data.to_string()))
              .with_default_spacer()
              .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
              .empty_if_not(|data, _| *data)
              .lens(ModEntry::utility),
          )
          .with_child(
            Flex::column()
              .with_child(
                Flex::row()
                  .with_child(h2_fixed("Total Conversion"))
                  .with_spacer(5.0)
                  .with_child(
                    Icon::new(*INFO)
                      .fix_size(15.0, 15.0)
                      .with_hover_state(false)
                      .stack_tooltip_custom(Card::new(
                        Flex::column()
                          .with_child(bolded(
                            "Total Conversion mods automatically disable all other mods that are",
                          ))
                          .with_child(bolded("not tagged as utility mods."))
                          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
                          .padding((7.0, 0.0)),
                      )),
                  ),
              )
              .with_child(Label::wrapped_func(|data: &bool, _| data.to_string()))
              .with_default_spacer()
              .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
              .empty_if_not(|data, _| *data)
              .lens(ModEntry::total_conversion),
          )
          .with_child(
            Flex::column()
              .with_child(h2_fixed("Dependencies"))
              .with_child(List::new(|| {
                hoverable_text_opts(
                  Some(ModDescription::DEP_TEXT_BG_COLOR),
                  identity,
                  &[druid::text::Attribute::Weight(
                    druid::text::FontWeight::SEMI_BOLD,
                  )],
                  &[druid::text::Attribute::TextColor(druid::KeyOrValue::Key(
                    ModDescription::DEP_TEXT_COLOR,
                  ))],
                  true,
                )
                .lens(Compute::new(ToString::to_string))
                .background(Painter::new(|ctx, _, env| {
                  use druid::RenderContext;

                  let size = ctx.size();
                  if ctx.is_hot() {
                    ctx.fill(size.to_rect(), &env.get(ModDescription::DEP_TEXT_BG_COLOR))
                  }
                }))
                .env_scope(|env, dep: &super::mod_entry::Dependency| {
                  let dep_map = env.get(ModDescription::DEP_MAP);
                  let status = dep_map.get(&dep.id);

                  let text_color = status.map_or_else(
                    || druid::theme::TEXT_COLOR.into(),
                    UpdateStatus::as_text_colour,
                  );
                  env.set(ModDescription::DEP_TEXT_COLOR, text_color.resolve(env));

                  let bg_color = status.map_or_else(
                    || druid::theme::BACKGROUND_LIGHT.into(),
                    KeyOrValue::<Color>::from,
                  );
                  env.set(ModDescription::DEP_TEXT_BG_COLOR, bg_color.resolve(env));
                })
                .on_click(|ctx, data, _| {
                  ctx.submit_notification(ModDescription::NOTIF_OPEN_DEP.with(data.id.clone()));
                })
                .controller(HoverController::new(false, true))
              }))
              .with_default_spacer()
              .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
              .empty_if(
                |data: &std::sync::Arc<Vec<super::mod_entry::Dependency>>, _| data.is_empty(),
              )
              .lens(ModEntry::dependencies),
          )
          .with_child(Flex::column().cross_axis_alignment(druid::widget::CrossAxisAlignment::Start))
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
              hoverable_text_opts(
                Some(theme::BLUE_KEY),
                identity,
                &[druid::text::Attribute::Weight(
                  druid::text::FontWeight::SEMI_BOLD,
                )],
                &[druid::text::Attribute::TextColor(druid::KeyOrValue::Key(
                  theme::ON_BLUE_KEY,
                ))],
                true,
              )
              .on_click(|ctx, data, _| ctx.submit_command(OPEN_IN_BROWSER.with(data.clone())))
            })
            .lens(Compute::new(ModEntry::fractal_link))
            .background(Painter::new(|ctx, _, env| {
              use druid::RenderContext;

              let size = ctx.size();
              if ctx.is_hot() {
                ctx.fill(size.to_rect(), &env.get(theme::BLUE_KEY))
              }
            }))
            .controller(HoverController::new(false, true)),
          )
          .with_default_spacer()
          .with_child(
            Maybe::or_empty(|| h2_fixed("NexusMods page")).lens(Compute::new(ModEntry::nexus_link)),
          )
          .with_spacer(4.0)
          .with_child(
            Maybe::or_empty(|| {
              hoverable_text_opts(
                Some(theme::BLUE_KEY),
                identity,
                &[druid::text::Attribute::Weight(
                  druid::text::FontWeight::SEMI_BOLD,
                )],
                &[druid::text::Attribute::TextColor(druid::KeyOrValue::Key(
                  theme::ON_BLUE_KEY,
                ))],
                true,
              )
              .on_click(|ctx, data, _| ctx.submit_command(OPEN_IN_BROWSER.with(data.clone())))
            })
            .lens(Compute::new(ModEntry::nexus_link))
            .background(Painter::new(|ctx, _, env| {
              use druid::RenderContext;

              let size = ctx.size();
              if ctx.is_hot() {
                ctx.fill(size.to_rect(), &env.get(theme::BLUE_KEY))
              }
            }))
            .controller(HoverController::new(false, true)),
          )
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .must_fill_main_axis(true)
          .scroll()
          .vertical()
          .expand()
          .padding(5.0),
      )
      .expand_height()
  }

  pub fn empty_builder() -> impl Widget<()> {
    Label::new("No mod selected.")
  }

  pub fn enable_dependencies(_: &mut druid::EventCtx, id: &String, data: &mut App) {
    let mods = &mut data.mod_list.mods;
    if let Some(entry) = mods.get(id).cloned() {
      if entry.dependencies.iter().all(|d| {
        mods.get(&d.id).is_some_and(|entry| match &d.version {
          Some(v) => v.major() == entry.version.major(),
          None => true,
        })
      }) {
        for dep in entry.dependencies.as_ref() {
          App::mod_list
            .then(ModList::mods)
            .index(&dep.id)
            .then(ModEntry::enabled.in_arc())
            .put(data, true);
        }
      } else {
        App::mod_list
          .then(ModList::mods)
          .index(&entry.id)
          .then(ModEntry::enabled.in_arc())
          .put(data, false);
      }
    }
  }
}

pub fn notify_enabled(
  ctx: &mut druid::EventCtx,
  _: &ModEntry,
  data: &mut ModEntry,
  _: &druid::Env,
) {
  if data.enabled {
    ctx.submit_notification(ENABLE_DEPENDENCIES.with(data.id.clone()))
  }
}

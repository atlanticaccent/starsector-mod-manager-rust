use chrono::{DateTime, Utc};
use druid::{
  lens, theme,
  widget::{Either, Flex, Label, Maybe, Painter, SizedBox, ViewSwitcher},
  Data, Lens, LensExt, RenderContext, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, wrap::Wrap, Separator};
use im::{HashMap, Vector};
use serde::Deserialize;
use tap::Tap;

use super::{
  controllers::HoverController,
  modal::Modal,
  util::{hoverable_text, icons::*, LabelExt, CommandExt}, mod_description::OPEN_IN_BROWSER,
};

#[derive(Deserialize, Data, Clone, Lens)]
pub struct ModRepo {
  #[data(same_fn = "PartialEq::eq")]
  items: Vector<ModRepoItem>,
  #[data(same_fn = "PartialEq::eq")]
  #[serde(alias = "lastUpdated")]
  last_updated: DateTime<Utc>,
}

impl ModRepo {
  const REPO_URL: &'static str =
    "https://raw.githubusercontent.com/davidwhitman/StarsectorModRepo/main/ModRepo.json";

  const CARD_MAX_WIDTH: f64 = 475.0;

  pub fn ui_builder() -> impl Widget<ModRepo> {
    Modal::new("Mod Repo")
      .with_content(
        ViewSwitcher::new(
          |data: &Vector<ModRepoItem>, _| data.len(),
          |len, _, _| {
            let mut wrap = Wrap::new()
              .direction(druid::widget::Axis::Horizontal)
              .alignment(druid_widget_nursery::wrap::WrapAlignment::SpaceAround)
              .run_alignment(druid_widget_nursery::wrap::WrapAlignment::SpaceAround)
              .cross_alignment(druid_widget_nursery::wrap::WrapCrossAlignment::Center);

            (0..*len).into_iter().for_each(|i| {
              wrap.add_child(
                ModRepoItem::ui_builder()
                  .lens(lens::Index::new(i))
                  .fix_width(Self::CARD_MAX_WIDTH)
                  .boxed(),
              )
            });

            wrap.expand_width().boxed()
          },
        )
        .lens(ModRepo::items)
        .boxed(),
      )
      .with_close()
      .build()
  }

  pub async fn get_mod_repo() -> anyhow::Result<Self> {
    let repo = reqwest::get(Self::REPO_URL)
      .await?
      .json::<ModRepo>()
      .await?;

    Ok(repo)
  }
}

#[derive(Deserialize, Data, Clone, PartialEq, Lens)]
pub struct ModRepoItem {
  name: String,
  #[serde(alias = "modVersion")]
  mod_version: Option<String>,
  #[serde(alias = "gameVersionReq")]
  game_version: Option<String>,
  summary: Option<String>,
  description: Option<String>,
  #[serde(skip)]
  show_description: bool,
  #[serde(rename = "authorsList")]
  #[data(same_fn = "PartialEq::eq")]
  authors: Vector<String>,
  #[data(same_fn = "PartialEq::eq")]
  urls: HashMap<ModSource, String>,
  #[data(same_fn = "PartialEq::eq")]
  sources: Vector<ModSource>,
  #[data(same_fn = "PartialEq::eq")]
  categories: Option<Vector<String>>,
  #[data(same_fn = "PartialEq::eq")]
  #[serde(alias = "dateTimeCreated")]
  created: Option<DateTime<Utc>>,
  #[data(same_fn = "PartialEq::eq")]
  #[serde(alias = "dateTimeEdited")]
  edited: Option<DateTime<Utc>>,
}

impl ModRepoItem {
  const CARD_INSET: f64 = 12.5;
  const LABEL_FLEX: f64 = 1.0;
  const VALUE_FLEX: f64 = 3.0;

  fn ui_builder() -> impl Widget<ModRepoItem> {
    Flex::column()
      .with_child(
        Flex::row()
          .with_flex_child(
            Label::new("Name:").align_right().expand_width(),
            Self::LABEL_FLEX,
          )
          .with_flex_child(Label::wrapped_lens(ModRepoItem::name), Self::VALUE_FLEX)
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .expand_width(),
      )
      .with_child(
        Maybe::or_empty(|| Separator::new().with_width(0.5).padding(5.)).lens(ModRepoItem::summary),
      )
      .with_child(
        Maybe::or_empty(|| {
          Flex::row()
            .with_flex_child(
              Label::new("Summary:").align_right().expand_width(),
              Self::LABEL_FLEX,
            )
            .with_flex_child(
              Label::wrapped_func(|data: &String, _| data.trim().to_string()),
              Self::VALUE_FLEX,
            )
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            .expand_width()
        })
        .lens(ModRepoItem::summary),
      )
      .with_child(
        Maybe::or_empty(|| Separator::new().with_width(0.5).padding(5.))
          .lens(ModRepoItem::description),
      )
      .with_child(ViewSwitcher::new(
        |data: &ModRepoItem, _| (data.description.clone(), data.show_description),
        |(description, show), _, _| {
          if let Some(description) = description {
            let row = Flex::row().with_flex_child(
              Flex::row()
                .with_child(Either::new(
                  |data, _| *data,
                  Icon::new(ARROW_DROP_DOWN),
                  Icon::new(ARROW_RIGHT),
                ))
                .with_child(Label::new("Description:"))
                .main_axis_alignment(druid::widget::MainAxisAlignment::End)
                .align_right()
                .expand_width()
                .controller(HoverController)
                .on_click(|_, data: &mut bool, _| *data = !*data)
                .lens(ModRepoItem::show_description)
                .padding((0., -2., 0., 0.)),
              Self::LABEL_FLEX,
            );

            if *show {
              row.with_flex_child(Label::wrapped(&description), Self::VALUE_FLEX)
            } else {
              row.with_flex_child(
                Label::new("Click to expand...")
                  .controller(HoverController)
                  .on_click(|_, data: &mut bool, _| *data = !*data)
                  .lens(ModRepoItem::show_description),
                Self::VALUE_FLEX,
              )
            }
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            .expand_width()
            .boxed()
          } else {
            SizedBox::empty().boxed()
          }
        },
      ))
      .with_child(
        Maybe::or_empty(|| Separator::new().with_width(0.5).padding(5.))
          .lens(ModRepoItem::authors.map(|data| (!data.is_empty()).then_some(()), |_, _| {})),
      )
      .with_child(
        Maybe::or_empty(|| {
          Flex::row()
            .with_flex_child(
              Label::new("Authors:").align_right().expand_width(),
              Self::LABEL_FLEX,
            )
            .with_flex_child(
              Label::wrapped_func(|data: &Vector<String>, _| {
                data
                  .iter()
                  .cloned()
                  .reduce(|acc, el| format!("{}, {}", acc, el))
                  .unwrap()
                  .trim()
                  .to_string()
              }),
              Self::VALUE_FLEX,
            )
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            .expand_width()
        })
        .lens(ModRepoItem::authors.map(|data| (!data.is_empty()).then(|| data.clone()), |_, _| {})),
      )
      .with_child(
        Maybe::or_empty(|| Separator::new().with_width(0.5).padding(5.))
          .lens(ModRepoItem::urls.map(|data| (!data.is_empty()).then_some(()), |_, _| {})),
      )
      .with_child(
        Maybe::or_empty(|| {
          Flex::row()
            .with_flex_child(
              Label::new("Links:").align_right().expand_width(),
              Self::LABEL_FLEX,
            )
            .with_flex_child(
              Flex::column()
                .with_child(
                  Maybe::or_empty(|| {
                    hoverable_text(Some(druid::Color::rgb8(0x00, 0x7B, 0xFF)))
                      .controller(HoverController)
                      .on_click(|ctx, data, _| {
                        ctx.submit_command_global(OPEN_IN_BROWSER.with(data.clone()))
                      })
                  })
                  .lens(lens::Map::new(
                    |data: &HashMap<ModSource, String>| data.get(&ModSource::Forum).cloned(),
                    |_, _| {},
                  ))
                  .align_left()
                  .expand_width(),
                )
                .with_child(
                  Maybe::or_empty(|| {
                    hoverable_text(Some(druid::Color::rgb8(0x00, 0x7B, 0xFF)))
                      .controller(HoverController)
                      .on_click(|_ctx, data, _| {
                        // ctx.submit_command_global(OPEN_IN_BROWSER.with(data.clone()))
                        let discord_uri = data.clone().tap_mut(|uri| uri.replace_range(0..5, "discord"));
                        let _ = opener::open(discord_uri);
                      })
                  })
                  .lens(lens::Map::new(
                    |data: &HashMap<ModSource, String>| data.get(&ModSource::Discord).cloned(),
                    |_, _| {},
                  ))
                  .align_left()
                  .expand_width(),
                )
                .with_child(
                  Maybe::or_empty(|| {
                    hoverable_text(Some(druid::Color::rgb8(0x00, 0x7B, 0xFF)))
                      .controller(HoverController)
                      .on_click(|ctx, data, _| {
                        ctx.submit_command_global(OPEN_IN_BROWSER.with(data.clone()))
                      })
                  })
                  .lens(lens::Map::new(
                    |data: &HashMap<ModSource, String>| data.get(&ModSource::NexusMods).cloned(),
                    |_, _| {},
                  ))
                  .align_left()
                  .expand_width(),
                ),
              Self::VALUE_FLEX,
            )
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            .expand_width()
        })
        .lens(ModRepoItem::urls.map(|data| (!data.is_empty()).then(|| data.clone()), |_, _| {})),
      )
      .padding(Self::CARD_INSET)
      .background(Painter::new(|ctx, _, env| {
        let size = ctx.size();

        let rounded_rect = size
          .to_rect()
          .inset(-Self::CARD_INSET / 2.0)
          .to_rounded_rect(10.);

        ctx.fill(rounded_rect, &env.get(theme::BACKGROUND_LIGHT));
      }))
      .expand_width()
  }
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModSource {
  Forum,
  ModdingSubforum,
  Discord,
  NexusMods,
  Index,
}

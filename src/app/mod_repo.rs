use std::fmt::Display;

use chrono::{DateTime, Local, Utc};
use deunicode::deunicode;
use druid::{
  im::{HashMap, Vector},
  lens::{self, Index},
  theme,
  widget::{Either, Flex, Label, Maybe, Painter, SizedBox, Spinner, TextBox, ViewSwitcher},
  Data, Lens, LensExt, Menu, MenuItem, RenderContext, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::{
  material_icons::Icon, wrap::Wrap, FutureWidget, Separator, WidgetExt as WidgetExtNursery,
};
use internment::Intern;
use rayon::iter::{ParallelBridge, ParallelIterator};
use serde::Deserialize;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use sublime_fuzzy::best_match;

use super::{
  controllers::HoverController,
  mod_description::OPEN_IN_BROWSER,
  modal::Modal,
  util::{
    default_true, hoverable_text, icons::*, xxHashMap, Button2, CommandExt, LabelExt, Tap as _,
    WidgetExtEx,
  },
  App,
};
use crate::widgets::{
  card::Card,
  card_button::CardButton,
  wrapped_table::{WrapData, WrappedTable},
};

#[derive(Deserialize, Data, Clone, Lens, Debug)]
pub struct ModRepo {
  #[serde(deserialize_with = "ModRepo::deserialize_items")]
  items: Vector<ModRepoItem>,
  #[data(same_fn = "PartialEq::eq")]
  #[serde(alias = "lastUpdated")]
  last_updated: DateTime<Utc>,
  #[serde(skip)]
  pub modal: Option<String>,
  #[serde(skip)]
  search: String,
  #[serde(skip)]
  filters: Vector<ModSource>,
  #[serde(skip)]
  #[serde(default = "ModRepo::default_sorting")]
  sort_by: Metadata,
  #[serde(skip)]
  #[serde(default = "ModRepo::default_page_size")]
  page_size: Option<usize>,
  #[serde(skip)]
  page_number: usize,
}

impl ModRepo {
  const REPO_URL: &'static str =
    "https://raw.githubusercontent.com/davidwhitman/StarsectorModRepo/main/ModRepo.json";

  pub const OPEN_IN_DISCORD: Selector = Selector::new("mod_repo.open.discord");
  const OPEN_CONFIRM: Selector<String> = Selector::new("mod_repo.open.discord.confirm");
  pub const CLEAR_MODAL: Selector = Selector::new("mod_repo.close.clear");
  const UPDATE_FILTERS: Selector<Filter> = Selector::new("mod_repo.filter.update");
  const UPDATE_SORTING: Selector<Metadata> = Selector::new("mod_repo.sorting.update");

  pub fn wrapper() -> impl Widget<App> {
    FutureWidget::new(
      |_, _| Self::get_mod_repo(),
      Spinner::new().valign_centre().halign_centre(),
      |mod_repo, app: &mut Option<ModRepo>, _| {
        *app = mod_repo.ok();

        Maybe::new(Self::view, || {
          Label::new("Could not load Starmodder catalogue")
        })
        .boxed()
      },
    )
    .lens(App::mod_repo)
  }

  pub fn view() -> impl Widget<ModRepo> {
    Flex::column()
      .with_child(Self::controls())
      .with_flex_child(
        WrappedTable::new(450.0, |_, id, _| {
          Card::new(ModRepoItem::view()).lens(ModRepo::items.index(id))
        })
        .scroll()
        .vertical()
        .expand_width(),
        1.0,
      )
      .expand_width()
  }

  pub fn controls() -> impl Widget<ModRepo> {
    const BUTTON_WIDTH: f64 = 250.0;

    Flex::row()
      .with_child(CardButton::button(|_| Label::new("Filters")).fix_width(BUTTON_WIDTH))
      .with_child(
        Button2::from_label("Filters").on_click2(|ctx, mouse, _, _| {
          let lens = App::mod_repo.map(
            |data| data.clone().unwrap(),
            |orig, new| {
              orig.replace(new);
            },
          );

          let menu = Menu::<App>::empty().pipe(|mut menu| {
            for source in [
              ModSource::Index,
              ModSource::ModdingSubforum,
              ModSource::Discord,
              ModSource::NexusMods,
            ] {
              menu = menu.entry(
                MenuItem::new(source.to_string())
                  .selected_if(move |data: &ModRepo, _| data.filters.contains(&source))
                  .on_activate(move |ctx, _, _| {
                    ctx.submit_command(Self::UPDATE_FILTERS.with(Filter::Source(source)))
                  })
                  .lens(lens.clone()),
              )
            }

            menu
          });

          ctx.show_context_menu(menu, ctx.to_window(mouse.pos))
        }),
      )
      .with_default_spacer()
      .with_child(
        Button2::from_label("Sort by").on_click2(|ctx, mouse, _, _| {
          let lens = App::mod_repo.map(
            |data| data.clone().unwrap(),
            |orig, new| {
              orig.replace(new);
            },
          );

          let menu = Menu::<App>::empty().pipe(|mut menu| {
            for meta in Metadata::iter().filter(|m| m != &Metadata::Score) {
              menu = menu.entry(
                MenuItem::new(meta.to_string())
                  .selected_if(move |data: &ModRepo, _| data.sort_by == meta)
                  .on_activate(move |ctx, _, _| {
                    ctx.submit_command(ModRepo::UPDATE_SORTING.with(meta))
                  })
                  .lens(lens.clone()),
              )
            }

            menu
          });

          ctx.show_context_menu(menu, ctx.to_window(mouse.pos))
        }),
      )
      .with_default_spacer()
      .with_child(Label::new("Search:").with_text_size(18.))
      .with_default_spacer()
      .with_child(
        TextBox::new()
          .on_change(|ctx, _: &String, data, _| {
            ctx.submit_command(ModRepo::UPDATE_FILTERS.with(Filter::Search(data.clone())))
          })
          .lens(ModRepo::search),
      )
      .main_axis_alignment(druid::widget::MainAxisAlignment::End)
      .expand_width()
  }

  pub async fn get_mod_repo() -> anyhow::Result<Self> {
    reqwest::get(Self::REPO_URL)
      .await?
      .json::<ModRepo>()
      .await
      .map_err(Into::into)

    // repo.items.sort_by(|a, b| Metadata::Name.comparator(a, b));
  }

  pub fn modal_open(&self) -> bool {
    self.modal.is_some()
  }

  fn default_sorting() -> Metadata {
    Metadata::Name
  }

  fn default_page_size() -> Option<usize> {
    Some(50)
  }

  fn deserialize_items<'de, D>(d: D) -> Result<Vector<ModRepoItem>, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    struct ItemVisitor;

    impl<'de> serde::de::Visitor<'de> for ItemVisitor {
      type Value = Vector<ModRepoItem>;

      fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "a sequence of items")
      }

      fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
      where
        A: serde::de::SeqAccess<'de>,
      {
        let mut list = Vector::new();

        while let Some(mut item) = seq.next_element::<ModRepoItem>()? {
          item.summary = item.summary.as_ref().map(|summary| deunicode(summary));
          item.description = item
            .description
            .as_ref()
            .map(|description| deunicode(description));
          item.name = deunicode(&item.name);

          list.push_back(item);
        }

        Ok(list)
      }
    }

    d.deserialize_seq(ItemVisitor)
  }
}

impl WrapData for ModRepo {
  type Id<'a> = usize;
  type OwnedId = usize;
  type Value = ModRepoItem;

  fn ids<'a>(&'a self) -> impl Iterator<Item = usize> {
    self
      .items
      .ids()
      .skip(self.page_number * self.page_size.unwrap_or_default())
      .take(self.page_size.unwrap_or(usize::MAX))
  }

  fn len(&self) -> usize {
    let prefix = self.page_number * self.page_size.unwrap_or_default();
    if self.items.len() >= prefix + self.page_size.unwrap_or_default() {
      self.page_size.unwrap_or(self.items.len())
    } else {
      self.items.len() - prefix
    }
  }
}

#[derive(Deserialize, Data, Clone, PartialEq, Eq, Lens, Debug)]
pub struct ModRepoItem {
  name: String,
  summary: Option<String>,
  description: Option<String>,
  #[serde(alias = "modVersion")]
  mod_version: Option<String>,
  #[serde(alias = "gameVersionReq")]
  game_version: Option<String>,
  #[serde(rename = "authorsList")]
  #[data(same_fn = "PartialEq::eq")]
  authors: Option<Vector<String>>,
  #[data(same_fn = "PartialEq::eq")]
  urls: Option<HashMap<UrlSource, String>>,
  #[data(same_fn = "PartialEq::eq")]
  sources: Option<Vector<ModSource>>,
  #[data(same_fn = "PartialEq::eq")]
  categories: Option<Vector<String>>,
  #[data(same_fn = "PartialEq::eq")]
  #[serde(alias = "dateTimeCreated")]
  created: Option<DateTime<Utc>>,
  #[data(same_fn = "PartialEq::eq")]
  #[serde(alias = "dateTimeEdited")]
  edited: Option<DateTime<Utc>>,
  #[serde(skip)]
  show_description: bool,
  #[serde(skip)]
  #[serde(default = "default_true")]
  display: bool,
  #[serde(skip)]
  score: Option<isize>,
}

impl ModRepoItem {
  const CARD_INSET: f64 = 12.5;
  const LABEL_FLEX: f64 = 1.0;
  const VALUE_FLEX: f64 = 3.0;

  fn view() -> impl Widget<ModRepoItem> {
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
                  Icon::new(*ARROW_DROP_DOWN),
                  Icon::new(*ARROW_RIGHT),
                ))
                .with_child(Label::new("Description:"))
                .main_axis_alignment(druid::widget::MainAxisAlignment::End)
                .align_right()
                .expand_width()
                .controller(HoverController::default())
                .on_click(|_, data: &mut bool, _| *data = !*data)
                .lens(ModRepoItem::show_description)
                .padding((0., -2., 0., 0.)),
              Self::LABEL_FLEX,
            );

            if *show {
              row.with_flex_child(Label::wrapped(description), Self::VALUE_FLEX)
            } else {
              row.with_flex_child(
                Label::new("Click to expand...")
                  .controller(HoverController::default())
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
        Maybe::or_empty(|| Separator::new().with_width(0.5).padding(5.)).lens(
          ModRepoItem::authors.map(
            |data| (data.as_ref().is_some_and(|data| !data.is_empty())).then_some(()),
            |_, _| {},
          ),
        ),
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
        .lens(ModRepoItem::authors.map(
          |data| {
            (data.as_ref().is_some_and(|data| !data.is_empty()))
              .then(|| data.clone())
              .flatten()
          },
          |_, _| {},
        )),
      )
      .with_child(
        Maybe::or_empty(|| Separator::new().with_width(0.5).padding(5.)).lens(
          ModRepoItem::urls.map(
            |data| (data.as_ref().is_some_and(|data| !data.is_empty())).then_some(()),
            |_, _| {},
          ),
        ),
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
                      .controller(HoverController::default())
                      .on_click(|ctx, data, _| {
                        ctx.submit_command_global(OPEN_IN_BROWSER.with(data.clone()))
                      })
                  })
                  .lens(lens::Map::new(
                    |data: &HashMap<UrlSource, String>| data.get(&UrlSource::Forum).cloned(),
                    |_, _| {},
                  ))
                  .align_left()
                  .expand_width(),
                )
                .with_child(
                  Maybe::or_empty(|| {
                    hoverable_text(Some(druid::Color::rgb8(0x00, 0x7B, 0xFF)))
                      .controller(HoverController::default())
                      .on_click(|ctx, data, _| {
                        ctx.submit_notification(ModRepo::OPEN_CONFIRM.with(data.clone()))
                      })
                  })
                  .lens(lens::Map::new(
                    |data: &HashMap<UrlSource, String>| data.get(&UrlSource::Discord).cloned(),
                    |_, _| {},
                  ))
                  .align_left()
                  .expand_width(),
                )
                .with_child(
                  Maybe::or_empty(|| {
                    hoverable_text(Some(druid::Color::rgb8(0x00, 0x7B, 0xFF)))
                      .controller(HoverController::default())
                      .on_click(|ctx, data, _| {
                        ctx.submit_command_global(OPEN_IN_BROWSER.with(data.clone()))
                      })
                  })
                  .lens(lens::Map::new(
                    |data: &HashMap<UrlSource, String>| data.get(&UrlSource::NexusMods).cloned(),
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
        .lens(ModRepoItem::urls.map(
          |data| {
            (data.as_ref().is_some_and(|data| !data.is_empty()))
              .then(|| data.clone())
              .flatten()
          },
          |_, _| {},
        )),
      )
      .with_child(
        Maybe::or_empty(|| {
          Flex::column()
            .with_child(Separator::new().with_width(0.5).padding(5.))
            .with_child(
              Flex::row()
                .with_flex_child(
                  Label::new("Updated at:").align_right().expand_width(),
                  Self::LABEL_FLEX,
                )
                .with_flex_child(
                  Label::wrapped_func(|data: &String, _| data.to_string()),
                  Self::VALUE_FLEX,
                )
                .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
                .expand_width(),
            )
        })
        .lens(ModRepoItem::edited.map(
          |date| {
            (*date).map(|date| {
              DateTime::<Local>::from(date)
                .format("%v %I:%M%p")
                .to_string()
            })
          },
          |_, _| {},
        )),
      )
      .with_child(
        Maybe::or_empty(|| {
          Flex::column()
            .with_child(Separator::new().with_width(0.5).padding(5.))
            .with_child(
              Flex::row()
                .with_flex_child(
                  Label::new("Created at:").align_right().expand_width(),
                  Self::LABEL_FLEX,
                )
                .with_flex_child(
                  Label::wrapped_func(|data: &String, _| data.to_string()),
                  Self::VALUE_FLEX,
                )
                .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
                .expand_width(),
            )
        })
        .lens(ModRepoItem::created.map(
          |date| {
            (*date).map(|date| {
              DateTime::<Local>::from(date)
                .format("%v %I:%M%p")
                .to_string()
            })
          },
          |_, _| {},
        )),
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

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Hash, Data, strum_macros::EnumString, Debug)]
pub enum ModSource {
  Forum,
  ModdingSubforum,
  Discord,
  NexusMods,
  Index,
}

impl Display for ModSource {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "{}",
      match self {
        ModSource::Forum | ModSource::ModdingSubforum => "Fractal Mod Forums",
        ModSource::Discord => "Discord",
        ModSource::NexusMods => "Nexus Mods",
        ModSource::Index => "Fractal Mod Index",
      }
    ))
  }
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Hash, Data, Debug)]
pub enum UrlSource {
  Forum,
  Discord,
  NexusMods,
  DirectDownload,
  DownloadPage,
}

impl Display for UrlSource {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "{}",
      match self {
        UrlSource::Forum => "Fractal Mod Forums",
        UrlSource::Discord => "Discord",
        UrlSource::NexusMods => "Nexus Mods",
        UrlSource::DirectDownload => "Raw Url",
        UrlSource::DownloadPage => "Other",
      }
    ))
  }
}

#[derive(Clone, PartialEq, Data)]
enum Filter {
  Source(ModSource),
  Search(String),
}

impl Display for Filter {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Filter::Source(source) => source.fmt(f),
      Filter::Search(_) => f.write_fmt(format_args!("Search")),
    }
  }
}

#[derive(Clone, Copy, Data, PartialEq, EnumIter, Debug)]
enum Metadata {
  Name,
  Created,
  Updated,
  Authors,
  Score,
}

impl Metadata {
  fn comparator(&self, left: &ModRepoItem, right: &ModRepoItem) -> std::cmp::Ordering {
    match self {
      Metadata::Name => left.name.cmp(&right.name),
      Metadata::Created => right.created.cmp(&left.created),
      Metadata::Updated => right
        .edited
        .or(right.created)
        .cmp(&left.edited.or(left.created)),
      Metadata::Authors => left.authors.cmp(&right.authors),
      Metadata::Score => right.score.cmp(&left.score),
    }
  }
}

impl Display for Metadata {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!(
      "{}",
      match self {
        Self::Name => "Name",
        Self::Created => "Created At",
        Self::Updated => "Updated At",
        Self::Authors => "Author(s)",
        Self::Score => unimplemented!(),
      }
    ))
  }
}

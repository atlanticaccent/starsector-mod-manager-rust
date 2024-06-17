use std::fmt::Display;

use chrono::{DateTime, Local, Utc};
use deunicode::deunicode;
use druid::{
  im::{HashMap, Vector},
  lens::{self, Index, Map},
  theme,
  widget::{Either, Flex, Label, Maybe, Painter, SizedBox, Spinner, ViewSwitcher},
  Data, Lens, LensExt, RenderContext, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::{
  material_icons::Icon, prism::OptionSome, FutureWidget, Separator, WidgetExt as WidgetExtNursery,
};
use itertools::Itertools;
use serde::Deserialize;
use strum::{IntoEnumIterator, VariantArray};
use strum_macros::{EnumIter, EnumString, IntoStaticStr, VariantArray};
use sublime_fuzzy::best_match;

use super::{
  controllers::HoverController,
  mod_description::OPEN_IN_BROWSER,
  mod_list::search::Search,
  util::{
    default_true, hoverable_text, icons::*, lensed_bold, CommandExt, Compute, LabelExt, WidgetExtEx,
  },
  App,
};
use crate::{
  app::util::Tap,
  widgets::{
    card::Card,
    card_button::CardButton,
    wrapped_table::{WrapData, WrappedTable},
  },
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
  #[serde(default = "ModRepo::default_source_filters")]
  filters: HashMap<ModSource, bool>,
  #[serde(skip)]
  sort_by: Metadata,
  #[serde(skip)]
  #[serde(default = "ModRepo::default_page_size")]
  page_size: Option<usize>,
  #[serde(skip)]
  page_number: usize,
}

const BUTTON_WIDTH: f64 = 175.0;

impl ModRepo {
  const REPO_URL: &'static str =
    "https://raw.githubusercontent.com/davidwhitman/StarsectorModRepo/main/ModRepo.json";

  pub const OPEN_IN_DISCORD: Selector = Selector::new("mod_repo.open.discord");
  const OPEN_CONFIRM: Selector<String> = Selector::new("mod_repo.open.discord.confirm");
  pub const CLEAR_MODAL: Selector = Selector::new("mod_repo.close.clear");
  const UPDATE_PAGE: Selector = Selector::new("mod_repo.page.update");

  pub fn wrapper() -> impl Widget<App> {
    FutureWidget::new(
      |_, _| Self::get_mod_repo(),
      Spinner::new()
        .fix_size(40.0, 40.0)
        .valign_centre()
        .halign_centre(),
      |mod_repo, app: &mut Option<ModRepo>, _| {
        let mut err = None;
        *app = mod_repo.inspect_err(|e| err = Some(e.to_string())).ok();

        Maybe::new(Self::view, move || {
          Flex::column()
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            .with_child(Label::new("Could not load Starmodder catalogue:"))
            .with_child(Label::new(err.clone().unwrap()))
            .halign_centre()
        })
        .boxed()
      },
    )
    .lens(App::mod_repo)
  }

  pub fn view() -> impl Widget<ModRepo> {
    Flex::column()
      .with_child(Self::controls().padding((0.0, 5.0, 10.0, 5.0)))
      .with_flex_child(
        WrappedTable::new(450.0, |_, id, _| {
          Card::new(ModRepoItem::view()).lens(ModRepo::items.index(id))
        })
        .on_command(Self::UPDATE_PAGE, |ctx, _, _| {
          ctx.request_update();
          ctx.request_layout();
          ctx.request_paint();
        })
        .scroll()
        .vertical()
        .expand_width(),
        1.0,
      )
      .expand_width()
  }

  pub fn controls() -> impl Widget<ModRepo> {
    Flex::row()
      .with_child(Self::page_control())
      .with_flex_spacer(1.0)
      .with_child(Self::filter_control())
      .with_default_spacer()
      .with_child(Self::sort_control())
      .with_default_spacer()
      .with_child(Self::search_control())
      .main_axis_alignment(druid::widget::MainAxisAlignment::End)
      .expand_width()
  }

  fn filter_control() -> impl Widget<ModRepo> {
    fn filter_heading<T: Data>(_: bool) -> impl Widget<T> {
      Flex::row()
        .with_child(CardButton::button_text("Filter by Source"))
        .with_child(Icon::new(*TUNE))
        .must_fill_main_axis(true)
        .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceEvenly)
    }

    fn checkbox() -> impl Widget<bool> {
      Icon::new(*CHECK_BOX_OUTLINE_BLANK).else_if(|data, _| *data, Icon::new(*ADD_BOX))
    }

    CardButton::stacked_dropdown(
      filter_heading,
      |hovered| {
        use crate::app::App;

        Flex::column()
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .with_child(filter_heading(hovered))
          .with_default_spacer()
          .tap(|column| {
            for (idx, source) in ModSource::visible_iter().enumerate() {
              column.add_child(
                Flex::row()
                  .with_child(checkbox())
                  .with_child(if source == ModSource::ModdingSubforum {
                    Flex::column()
                      .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
                      .with_child(CardButton::button_text("Modding"))
                      .with_child(CardButton::button_text("Subforum"))
                      .boxed()
                  } else {
                    CardButton::button_text(source.into()).boxed()
                  })
                  .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
                  .on_click(move |_, data, _| {
                    *data = !*data;
                  })
                  .padding((3.0, 0.0))
                  .expand_width()
                  .lens(ModRepo::filters.index(&ModSource::VARIANTS[idx])),
              )
            }
          })
          .expand_width()
          .on_change(|_, _, repo, _| {
            let filters = &repo.filters;
            for item in repo.items.iter_mut() {
              item.display = filters.values().all_equal_value().is_ok()
                || item.sources.iter().flatten().any(|s| filters[s]);
            }
          })
          .prism(OptionSome)
          .lens(App::mod_repo)
      },
      BUTTON_WIDTH,
    )
  }

  fn sort_control() -> impl Widget<ModRepo> {
    fn radio_button() -> impl Widget<bool> {
      Icon::new(*RADIO_BUTTON_UNCHECKED).else_if(|data, _| *data, Icon::new(*RADIO_BUTTON_CHECKED))
    }

    fn sort_heading<T: Data>(_: bool) -> impl Widget<T> {
      Flex::row()
        .with_child(CardButton::button_text("Sort by"))
        .with_child(Icon::new(*SORT))
        .must_fill_main_axis(true)
        .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceEvenly)
    }

    CardButton::stacked_dropdown(
      sort_heading,
      |hovered| {
        use crate::app::App;

        Flex::column()
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .with_child(sort_heading(hovered))
          .with_default_spacer()
          .tap(|column| {
            let mut inner =
              Flex::column().cross_axis_alignment(druid::widget::CrossAxisAlignment::Start);
            for (idx, meta) in Metadata::visible_iter().enumerate() {
              inner.add_child(
                Flex::row()
                  .with_child(radio_button())
                  .with_child(CardButton::button_text(meta.into()))
                  .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
                  .on_click(move |_, data, _| {
                    *data = !*data;
                  })
                  .lens(Index::new(&Metadata::VARIANTS[idx]))
                  .on_change(move |_, _: &HashMap<_, _>, data, _| {
                    if data[&meta] {
                      data
                        .iter_mut()
                        .filter(|(m, _)| **m != meta)
                        .for_each(|(_, active)| *active = false);
                    }
                  }),
              )
            }
            column.add_child(inner)
          })
          .expand_width()
          .lens(Map::new(
            |repo: &ModRepo| {
              Metadata::visible_iter()
                .map(|m| (m, repo.sort_by == m))
                .collect::<HashMap<_, _>>()
            },
            |repo, sorts| {
              if repo.sort_by != Metadata::Score {
                repo.sort_by = sorts
                  .into_iter()
                  .find_map(|(s, active)| active.then_some(s))
                  .unwrap_or_default()
              }
            },
          ))
          .on_change(|_, _, repo, _| {
            let sort_by = &repo.sort_by;
            repo.items.sort_by(|a, b| sort_by.comparator(a, b))
          })
          .prism(OptionSome)
          .lens(App::mod_repo)
      },
      BUTTON_WIDTH,
    )
  }

  fn search_control() -> impl Widget<ModRepo> {
    Search::view()
      .lens(ModRepo::search)
      .on_change(|_, old, repo, _| {
        let search = &repo.search;

        let sort_by = if search.is_empty() && !old.search.is_empty() {
          Metadata::default()
        } else {
          for item in repo.items.iter_mut() {
            item.score = Some(&item.name)
              .into_iter()
              .chain(item.authors.iter().flatten())
              // .chain(item.categories.iter().flatten())
              .chain(item.description.iter())
              .filter_map(|t| best_match(&search, &t))
              .map(|m| m.score())
              .reduce(isize::max);
          }
          Metadata::Score
        };
        repo.sort_by = sort_by;
        repo.items.sort_by(|a, b| sort_by.comparator(a, b));
      })
  }

  fn page_control() -> impl Widget<ModRepo> {
    #[derive(Clone, Data, Lens)]
    struct PageState {
      page_number: usize,
      total_pages: usize,
      page_size: Option<usize>,
    }

    let is_start = |data: &PageState, _: &_| data.page_number == 0;
    let is_end = |data: &PageState, _: &_| data.page_number == data.total_pages - 1;
    let show_if = |data: &PageState, _: &_| data.total_pages > 1;

    Flex::row()
      .with_child(
        CardButton::button(|_| Icon::new(*DOUBLE_LEFT))
          .on_click(|_, data: &mut PageState, _| data.page_number = 0)
          .disabled_if(is_start)
          .env_scope(move |env, data| {
            if is_start(data, env) {
              env.set(
                druid::theme::TEXT_COLOR,
                env.get(druid::theme::DISABLED_TEXT_COLOR),
              )
            }
          })
          .or_empty(show_if),
      )
      .with_child(
        CardButton::button(|_| Icon::new(*CHEVRON_LEFT))
          .on_click(|_, data: &mut PageState, _| data.page_number -= 1)
          .env_scope(move |env, data| {
            if is_start(data, env) {
              env.set(
                druid::theme::TEXT_COLOR,
                env.get(druid::theme::DISABLED_TEXT_COLOR),
              )
            }
          })
          .disabled_if(is_start)
          .or_empty(show_if),
      )
      .with_child(
        CardButton::button(|_| {
          lensed_bold(
            druid::theme::TEXT_SIZE_NORMAL,
            druid::FontWeight::SEMI_BOLD,
            druid::theme::TEXT_COLOR,
          )
          .lens(Compute::new(|state: &PageState| {
            format!("{} / {}", state.page_number + 1, state.total_pages)
          }))
          .valign_centre()
        })
        .disabled(),
      )
      .with_child(
        CardButton::button(|_| Icon::new(*CHEVRON_RIGHT))
          .on_click(|_, data: &mut PageState, _| data.page_number += 1)
          .env_scope(move |env, data| {
            if is_end(data, env) {
              env.set(
                druid::theme::TEXT_COLOR,
                env.get(druid::theme::DISABLED_TEXT_COLOR),
              )
            }
          })
          .disabled_if(is_end)
          .or_empty(show_if),
      )
      .with_child(
        CardButton::button(|_| Icon::new(*DOUBLE_RIGHT))
          .on_click(|_, data: &mut PageState, _| data.page_number = data.total_pages - 1)
          .env_scope(move |env, data| {
            if is_end(data, env) {
              env.set(
                druid::theme::TEXT_COLOR,
                env.get(druid::theme::DISABLED_TEXT_COLOR),
              )
            }
          })
          .disabled_if(is_end)
          .or_empty(show_if),
      )
      .lens(Map::new(
        |repo: &ModRepo| {
          let total_pages = if let Some(page_size) = repo.page_size {
            (repo.items.iter().filter(|item| item.display).count() as f32 / page_size as f32).ceil()
              as usize
          } else {
            1
          };

          PageState {
            page_number: repo.page_number,
            total_pages,
            page_size: repo.page_size,
          }
        },
        |repo, state| repo.page_number = state.page_number,
      ))
      .on_change(|ctx, _, _, _| ctx.submit_command(Self::UPDATE_PAGE))
  }

  pub async fn get_mod_repo() -> anyhow::Result<Self> {
    let client = reqwest::Client::builder()
      .timeout(std::time::Duration::from_millis(500))
      .no_trust_dns()
      .build()?;

    let mut res;
    loop {
      res = client.get(Self::REPO_URL).send().await;
      if !res.as_ref().is_err_and(|err| err.is_timeout()) {
        break;
      } else {
        eprintln!("timeout")
      }
    }

    let mut repo = res
      .inspect_err(|err| {
        dbg!(err);
      })?
      .json::<ModRepo>()
      .await
      .map_err(|e| anyhow::anyhow!(e))?;

    repo
      .items
      .sort_by(|a, b| Metadata::default().comparator(a, b));

    Ok(repo)
  }

  pub fn modal_open(&self) -> bool {
    self.modal.is_some()
  }

  fn default_page_size() -> Option<usize> {
    Some(50)
  }

  fn default_source_filters() -> HashMap<ModSource, bool> {
    ModSource::iter().map(|s| (s, false)).collect()
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
      .iter()
      .enumerate()
      .filter_map(|(idx, item)| item.display.then_some(idx))
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
  authors: Option<Vector<String>>,
  urls: Option<HashMap<UrlSource, String>>,
  sources: Option<Vector<ModSource>>,
  categories: Option<Vector<String>>,
  #[data(eq)]
  #[serde(alias = "dateTimeCreated")]
  created: Option<DateTime<Utc>>,
  #[data(eq)]
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

#[derive(
  Deserialize,
  Clone,
  Copy,
  PartialEq,
  Eq,
  Hash,
  Data,
  EnumString,
  IntoStaticStr,
  EnumIter,
  VariantArray,
  Debug,
)]
pub enum ModSource {
  #[strum(to_string = "Forum Index")]
  Index,
  #[strum(to_string = "Modding Subforum")]
  ModdingSubforum,
  Discord,
  #[strum(to_string = "Nexus Mods")]
  NexusMods,
  #[strum(to_string = "Mod Forum")]
  Forum,
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

impl ModSource {
  fn visible_iter() -> impl Iterator<Item = Self> {
    Self::iter().filter(|s| *s != Self::Forum)
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

#[derive(
  Hash, Clone, Copy, Data, PartialEq, Eq, EnumIter, VariantArray, IntoStaticStr, Debug, Default,
)]
enum Metadata {
  Name,
  #[strum(to_string = "Created At")]
  Created,
  #[default]
  #[strum(to_string = "Updated At")]
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

  fn visible_iter() -> impl Iterator<Item = Self> {
    Self::iter().filter(|m| *m != Self::Score)
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

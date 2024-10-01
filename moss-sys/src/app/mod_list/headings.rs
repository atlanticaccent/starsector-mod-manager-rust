use std::hash::Hash;

use druid::{
  im::Vector,
  widget::{Controller, Flex, Label, Painter, ViewSwitcher},
  Data, Lens, RenderContext, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as WidgetExtNursery};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use super::{util::icons::{ARROW_DROP_DOWN, ARROW_DROP_UP, UNFOLD_MORE}, ModList};
use crate::{app::util::LabelExt, patch::split::Split};

#[derive(
  Debug,
  Clone,
  Copy,
  Data,
  PartialEq,
  Eq,
  Hash,
  EnumIter,
  Serialize,
  Deserialize,
  strum_macros::Display,
  strum_macros::IntoStaticStr,
  Default,
)]
pub enum Heading {
  ID,
  #[default]
  Name,
  #[strum(serialize = "Author(s)")]
  Author,
  #[strum(serialize = "Game Version")]
  GameVersion,
  Enabled,
  Version,
  Score,
  #[strum(serialize = "Auto-Update Supported")]
  AutoUpdateSupport,
  #[strum(serialize = "Install Date")]
  InstallDate,
  Type,
}

impl Heading {
  #[must_use] pub fn visible(&self) -> bool {
    matches!(self, Heading::Enabled | Heading::Score)
  }

  #[must_use] pub fn complete(list: &Vector<Heading>) -> bool {
    Heading::iter()
      .filter(|h| !h.visible())
      .all(|h| list.contains(&h))
  }
}

#[derive(Clone, Data, Lens, Default)]
pub struct Header {
  pub ratios: Vector<f64>,
  pub headings: Vector<Heading>,
  pub sort_by: (Heading, bool),
}

impl Header {
  pub const SWAP_HEADINGS: Selector<(usize, usize)> = Selector::new("headings.order.changed");
  pub const ADD_HEADING: Selector<Heading> = Selector::new("headings.add");
  pub const REMOVE_HEADING: Selector<Heading> = Selector::new("headings.remove");

  pub const ENABLED_WIDTH: f64 = 90.0;

  pub const TITLES: [Heading; 6] = [
    Heading::Name,
    Heading::ID,
    Heading::Author,
    Heading::Version,
    Heading::AutoUpdateSupport,
    Heading::GameVersion,
  ];

  #[must_use] pub fn new(headings: Vector<Heading>) -> Self {
    Self {
      ratios: Self::calculate_ratios(headings.len()),
      headings,
      sort_by: (Heading::Name, false),
    }
  }

  fn calculate_ratios(num_headings: usize) -> Vector<f64> {
    (0..num_headings - 1)
      .rev()
      .map(|idx| 1. / (idx + 2) as f64)
      .collect()
  }

  #[must_use] pub fn view() -> impl Widget<Header> {
    fn recursive_split(idx: usize, titles: &Vector<Heading>) -> impl Widget<Header> {
      if idx < titles.len() - 2 {
        Split::columns(
          heading_builder(titles[idx]).controller(ResizeController::new(idx + 1)),
          recursive_split(idx + 1, titles),
        )
      } else {
        Split::columns(
          heading_builder(titles[idx]).controller(ResizeController::new(idx + 1)),
          heading_builder(titles[idx + 1]).controller(ResizeController::new(idx + 2)),
        )
      }
      .draggable(true)
      .split_point(1. / (titles.len() - idx) as f64)
      .bar_size(0.0)
      .solid_bar(true)
      .min_size(50., 50.)
    }

    ViewSwitcher::new(
      |data: &Header, _| data.headings.clone(),
      |_, data, _| {
        Flex::row()
          .with_child(heading_builder(Heading::Enabled).fix_width(Self::ENABLED_WIDTH))
          .with_flex_child(
            if data.headings.len() > 1 {
              recursive_split(0, &data.headings).boxed()
            } else {
              heading_builder(data.headings[0]).boxed()
            }
            .expand_width(),
            1.0,
          )
          .boxed()
      },
    )
    .on_command(Header::SWAP_HEADINGS, |_, (idx, jdx), header| {
      header.headings.swap(*idx, *jdx);
    })
    .on_command(Header::ADD_HEADING, |ctx, heading, header| {
      header.headings.push_back(*heading);
      header.ratios = Self::calculate_ratios(header.headings.len());
      for (idx, ratio) in header.ratios.iter().enumerate() {
        ctx.submit_command(ModList::UPDATE_COLUMN_WIDTH.with((idx + 1, *ratio)));
      }
      ctx.submit_command(crate::app::controllers::HeightLinker::HEIGHT_LINKER_RESET_ALL);
      ctx.submit_command(ModList::REBUILD_NEXT_PASS);
    })
    .on_command(Header::REMOVE_HEADING, |ctx, heading, header| {
      header.headings.retain(|existing| existing != heading);
      if header.sort_by.0 == *heading
        && let Some(new_sort) = header.headings.iter().find(|h| h.visible())
      {
        header.sort_by.0 = *new_sort;
      }
      header.ratios = Self::calculate_ratios(header.headings.len());
      for (idx, ratio) in header.ratios.iter().enumerate() {
        ctx.submit_command(ModList::UPDATE_COLUMN_WIDTH.with((idx + 1, *ratio)));
      }
      ctx.submit_command(crate::app::controllers::HeightLinker::HEIGHT_LINKER_RESET_ALL);
      ctx.submit_command(ModList::REBUILD_NEXT_PASS);
    })
  }
}

impl Hash for Header {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.headings.hash(state);
    self.sort_by.hash(state);
  }
}

impl PartialEq for Header {
  fn eq(&self, other: &Self) -> bool {
    self.ratios == other.ratios && self.headings == other.headings && self.sort_by == other.sort_by
  }
}

impl Eq for Header {}

fn heading_builder(title: Heading) -> impl Widget<Header> {
  Flex::row()
    .with_flex_child(
      Label::wrapped(<&'static str>::from(title))
        .with_text_alignment(druid::TextAlignment::Center)
        .expand_width(),
      1.,
    )
    .with_child(
      ViewSwitcher::new(
        |data: &(Heading, bool), _| *data,
        move |_, new, _| {
          if new.0 == title {
            if new.1 {
              Box::new(Icon::new(*ARROW_DROP_DOWN))
            } else {
              Box::new(Icon::new(*ARROW_DROP_UP))
            }
          } else {
            Box::new(Icon::new(*UNFOLD_MORE))
          }
        },
      )
      .lens(Header::sort_by),
    )
    .fix_height(40.)
    .padding((0., 5., 0., 5.))
    .background(Painter::new(|ctx, _, env| {
      let border_rect = ctx.size().to_rect().inset(-1.5);
      if ctx.is_hot() {
        ctx.stroke(border_rect, &env.get(druid::theme::BORDER_LIGHT), 3.);
      }
    }))
    .on_click(move |ctx, data: &mut Header, _| {
      if data.sort_by.0 == title {
        data.sort_by.1 = !data.sort_by.1;
      } else {
        data.sort_by = (title, false);
      }
      ctx.submit_command(ModList::UPDATE_TABLE_SORT);
    })
}

struct ResizeController {
  id: usize,
}

impl ResizeController {
  fn new(id: usize) -> Self {
    Self { id }
  }
}

impl<W: Widget<Header>> Controller<Header, W> for ResizeController {
  fn lifecycle(
    &mut self,
    child: &mut W,
    ctx: &mut druid::LifeCycleCtx,
    event: &druid::LifeCycle,
    data: &Header,
    env: &druid::Env,
  ) {
    if let druid::LifeCycle::Size(size) = event {
      ctx.submit_command(ModList::UPDATE_COLUMN_WIDTH.with((self.id, size.width)));
    }

    child.lifecycle(ctx, event, data, env);
  }
}

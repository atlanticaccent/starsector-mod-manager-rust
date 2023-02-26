use crate::{
  app::{mod_entry::ModEntry, util::LabelExt},
  patch::split::{Split, DRAGGED},
};
use druid::{
  im::Vector,
  widget::{Controller, ControllerHost, Flex, Label, Painter, ViewSwitcher},
  Data, Lens, RenderContext, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as WidgetExtNursery};
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use super::util::icons::*;

pub const ENABLED_RATIO: f64 = 1. / 12.;

#[derive(Debug, Clone, Copy, Data, PartialEq, Eq, EnumIter, Serialize, Deserialize)]
pub enum Heading {
  ID,
  Name,
  Author,
  GameVersion,
  Enabled,
  Version,
  Score,
  AutoUpdateSupport,
  InstallDate,
}

impl From<Heading> for &str {
  fn from(sorting: Heading) -> Self {
    match sorting {
      Heading::ID => "ID",
      Heading::Name => "Name",
      Heading::Author => "Author(s)",
      Heading::GameVersion => "Game Version",
      Heading::Enabled => "Enabled",
      Heading::Version => "Version",
      Heading::Score => "score",
      Heading::AutoUpdateSupport => "Auto-Update Supported",
      Heading::InstallDate => "Install Date",
    }
  }
}

#[derive(Clone, Data, Lens)]
pub struct Header {
  #[data(same_fn = "PartialEq::eq")]
  pub ratios: Vector<f64>,
  #[data(same_fn = "PartialEq::eq")]
  pub headings: Vector<Heading>,
  pub sort_by: (Heading, bool),
}

impl Header {
  pub const SORT_CHANGED: Selector<Heading> = Selector::new("headings.sorting.changed");
  pub const SWAP_HEADINGS: Selector<(usize, usize)> = Selector::new("headings.order.changed");
  pub const ADD_HEADING: Selector<Heading> = Selector::new("headings.add");
  pub const REMOVE_HEADING: Selector<Heading> = Selector::new("headings.remove");

  pub const TITLES: [Heading; 6] = [
    Heading::Name,
    Heading::ID,
    Heading::Author,
    Heading::Version,
    Heading::AutoUpdateSupport,
    Heading::GameVersion,
  ];

  pub fn new(headings: Vector<Heading>) -> Self {
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

  pub fn ui_builder() -> impl Widget<Header> {
    fn recursive_split(
      idx: usize,
      titles: &Vector<Heading>,
    ) -> ControllerHost<Split<Header>, ResizeController> {
      if idx < titles.len() - 2 {
        Split::columns(
          heading_builder(titles[idx]),
          recursive_split(idx + 1, titles),
        )
      } else {
        Split::columns(
          heading_builder(titles[idx]),
          heading_builder(titles[idx + 1]),
        )
      }
      .draggable(true)
      .split_point(1. / (titles.len() - idx) as f64)
      .bar_size(2.)
      .solid_bar(true)
      .min_size(50., 50.)
      .controller(ResizeController::new(idx + 1))
    }

    ViewSwitcher::new(
      |data: &Header, _| data.headings.clone(),
      |_, data, _| {
        Split::columns(
          heading_builder(Heading::Enabled),
          if data.headings.len() > 1 {
            recursive_split(0, &data.headings).boxed()
          } else {
            heading_builder(data.headings[0]).boxed()
          },
        )
        .split_point(ENABLED_RATIO)
        .controller(ResizeController::new(0))
        .boxed()
      },
    )
    .on_command(Header::SWAP_HEADINGS, |_, (idx, jdx), header| {
      header.headings.swap(*idx, *jdx)
    })
    .on_command(Header::ADD_HEADING, |_, heading, header| {
      header.headings.push_back(*heading);
      header.ratios = Self::calculate_ratios(header.headings.len());
    })
    .on_command(Header::REMOVE_HEADING, |_, heading, header| {
      header.headings.retain(|existing| existing != heading);
      header.ratios = Self::calculate_ratios(header.headings.len());
    })
  }
}

fn heading_builder(title: Heading) -> impl Widget<Header> {
  Flex::row()
    .with_flex_child(
      Label::wrapped(<&str>::from(title))
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
              Box::new(Icon::new(ARROW_DROP_DOWN))
            } else {
              Box::new(Icon::new(ARROW_DROP_UP))
            }
          } else {
            Box::new(Icon::new(UNFOLD_MORE))
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
        ctx.stroke(border_rect, &env.get(druid::theme::BORDER_LIGHT), 3.)
      }
    }))
    .on_click(move |ctx, _, _| ctx.submit_command(Header::SORT_CHANGED.with(title)))
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
  fn event(
    &mut self,
    child: &mut W,
    ctx: &mut druid::EventCtx,
    event: &druid::Event,
    data: &mut Header,
    env: &druid::Env,
  ) {
    if let druid::Event::Notification(notif) = event {
      if let Some(ratio) = notif.get(DRAGGED) {
        ctx.set_handled();
        data.ratios[self.id] = *ratio;
        ctx.submit_command(ModEntry::UPDATE_RATIOS.with((self.id, *ratio)))
      }
    }
    child.event(ctx, event, data, env)
  }
}

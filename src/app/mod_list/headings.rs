use crate::{
  app::{mod_entry::ModEntry, util::LabelExt},
  patch::split::{Split, DRAGGED},
};
use druid::{
  widget::{ClipBox, Controller, ControllerHost, Flex, Label, Painter, ViewSwitcher},
  Data, Lens, RenderContext, Selector, UnitPoint, Widget, WidgetExt, im::Vector,
};
use druid_widget_nursery::material_icons::Icon;

use super::util::icons::*;

pub const RATIOS: [f64; 5] = [1. / 6., 1. / 5., 1. / 4., 1. / 3., 1. / 2.];
pub const ENABLED_RATIO: f64 = 1. / 12.;

#[derive(Debug, Clone, Copy, Data, PartialEq, Eq)]
pub enum Heading {
  ID,
  Name,
  Author,
  GameVersion,
  Enabled,
  Version,
  Score,
  AutoUpdateSupport,
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
    }
  }
}

#[derive(Clone, Data, Lens)]
pub struct Header {
  #[data(same_fn = "PartialEq::eq")]
  pub ratios: Vec<f64>,
  #[data(same_fn = "PartialEq::eq")]
  pub headings: Vector<Heading>,
  pub sort_by: (Heading, bool),
}

impl Header {
  pub const SORT_CHANGED: Selector<Heading> = Selector::new("headings.sorting.changed");

  const TITLES: [Heading; 6] = [
    Heading::Name,
    Heading::ID,
    Heading::Author,
    Heading::Version,
    Heading::AutoUpdateSupport,
    Heading::GameVersion,
  ];

  pub fn new(ratios: &[f64; 5]) -> Self {
    Self {
      ratios: ratios.to_vec(),
      headings: Header::TITLES.to_vec().into(),
      sort_by: (Heading::Name, false),
    }
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
          recursive_split(0, &data.headings),
        )
        .split_point(ENABLED_RATIO)
        .controller(ResizeController::new(0))
        .boxed()
      }
    )
  }
}

fn heading_builder(title: Heading) -> impl Widget<Header> {
  ClipBox::unmanaged(
    Flex::row()
      .with_child(Label::wrapped(<&str>::from(title)))
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
      ),
  )
  .constrain_horizontal(true)
  .align_vertical(UnitPoint::CENTER)
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

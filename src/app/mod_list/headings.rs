use druid::{Widget, widget::{Label, Controller, ClipBox, ControllerHost}, WidgetExt, Data, Lens, UnitPoint};
use crate::{patch::split::{Split, DRAGGED}, app::mod_entry::ModEntry};

pub const RATIOS: [f64; 5] = [
  1. / 6.,
  1. / 5.,
  1. / 4.,
  1. / 3.,
  1. / 2.
];

#[derive(Clone, Data, Lens)]
pub struct Headings {
  #[data(same_fn="PartialEq::eq")]
  pub ratios: [f64; 5]
}

impl Headings {
  const TITLES: [&'static str; 6] = ["Name", "ID", "Author(s)", "Version", "Auto-Update Supported?", "Game Version"];

  pub fn new(ratios: &[f64; 5]) -> Self {
    Self {
      ratios: ratios.clone()
    }
  }

  pub fn ui_builder() -> impl Widget<Headings> {
    fn recursive_split(idx: usize, titles: &[&str]) -> ControllerHost<Split<Headings>, ResizeController> {
      if idx < titles.len() - 2 {
        Split::columns(
          ClipBox::new(
            Label::new(titles[idx])
              .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
            )
          .constrain_horizontal(true)
          .align_vertical(UnitPoint::CENTER)
          .fix_height(40.)
          .padding((0., 5., 0., 5.)),
          recursive_split(idx + 1, titles)
        )
      } else {
        Split::columns(
          ClipBox::new(
            Label::new(titles[idx])
              .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
            )
          .constrain_horizontal(true)
          .align_vertical(UnitPoint::CENTER)
          .fix_height(40.)
          .padding((0., 5., 0., 5.)),
          ClipBox::new(
            Label::new(titles[idx + 1])
              .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
            )
          .constrain_horizontal(true)
          .align_vertical(UnitPoint::CENTER)
          .fix_height(40.)
          .padding((0., 5., 0., 5.))
        )
      }.draggable(true)
      .split_point(1. / (titles.len() - idx) as f64)
      .bar_size(2.)
      .solid_bar(true)
      .min_size(50., 50.)
      .controller(ResizeController::new(idx + 1))
    }

    Split::columns(
      Label::new("Enabled")
        .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
        .align_vertical(UnitPoint::CENTER)
        .fix_height(40.)
        .padding((0., 5., 0., 5.)),
      recursive_split(0, &Headings::TITLES)
    ).split_point(1. / 7.).controller(ResizeController::new(0))
  }
}

struct ResizeController {
  id: usize,
}

impl ResizeController {
  fn new(id: usize) -> Self {
    Self {
      id,
    }
  }
}

impl<W: Widget<Headings>> Controller<Headings, W> for ResizeController {
  fn event(&mut self, child: &mut W, ctx: &mut druid::EventCtx, event: &druid::Event, data: &mut Headings, env: &druid::Env) {
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

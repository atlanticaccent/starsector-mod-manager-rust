use std::{cell::RefCell, rc::Rc};

use druid::{BoxConstraints, Data, Selector, Size, Widget, WidgetId, WidgetPod};
use druid_widget_nursery::CommandCtx;

pub struct HeightLinker {
  pub linked: usize,
  pub resolved: usize,
  pub max: f64,
  id: WidgetId,
  only_once: bool,
}

pub type HeightLinkerShared = Rc<RefCell<HeightLinker>>;

enum HeightLinkerCmd {
  SetHeight(f64),
  ResetHeight,
}

impl HeightLinker {
  const HEIGHT_LINKER_CMD: Selector<(WidgetId, HeightLinkerCmd)> = Selector::new("height_linker.command");

  fn increment_resolved(&mut self, ctx: &mut impl CommandCtx, height: f64) {
    self.resolved += 1;
    if self.resolved <= self.linked && height > self.max {
      self.max = height;
      ctx.submit_command(
        Self::HEIGHT_LINKER_CMD.with((self.id, HeightLinkerCmd::SetHeight(self.max))),
      );
    }
  }

  fn resolved(&self) -> bool {
    self.resolved >= self.linked
  }

  fn reset(&mut self, ctx: &mut impl CommandCtx) {
    if !self.only_once {
      self.resolved = 0;
      self.max = f64::NEG_INFINITY;
      ctx.submit_command(Self::HEIGHT_LINKER_CMD.with((self.id, HeightLinkerCmd::ResetHeight)))
    }
  }

  pub fn only_once(&mut self) {
    self.only_once = true;
  }
}

pub struct LinkedHeights<T: Data, W: Widget<T>> {
  widget: WidgetPod<T, W>,
  height_linker: HeightLinkerShared,
  height: Option<f64>,
  last_unconstrained: Option<f64>,
}

impl<T: Data, W: Widget<T>> LinkedHeights<T, W> {
  pub fn new(widget: W, height_linker: HeightLinkerShared) -> Self {
    height_linker.borrow_mut().linked += 1;
    Self {
      widget: WidgetPod::new(widget),
      height_linker,
      height: None,
      last_unconstrained: None,
    }
  }

  pub fn new_with_linker(widget: W) -> (Self, HeightLinkerShared) {
    let linker = Rc::new(RefCell::new(HeightLinker {
      linked: 0,
      resolved: 0,
      max: f64::NEG_INFINITY,
      id: WidgetId::next(),
      only_once: false,
    }));

    let this = Self::new(widget, linker.clone());

    (this, linker)
  }
}

impl<T: Data, W: Widget<T>> Widget<T> for LinkedHeights<T, W> {
  fn event(
    &mut self,
    ctx: &mut druid::widget::prelude::EventCtx,
    event: &druid::widget::prelude::Event,
    data: &mut T,
    env: &druid::widget::prelude::Env,
  ) {
    if let druid::Event::Command(cmd) = event {
      if let Some((id, cmd)) = cmd.get(HeightLinker::HEIGHT_LINKER_CMD) {
        if self.height_linker.borrow().id == *id {
          match cmd {
            HeightLinkerCmd::SetHeight(height) => {
              self.height = Some(*height);
              ctx.request_layout()
            }
            HeightLinkerCmd::ResetHeight => {
              self.height = None;
              ctx.request_layout()
            },
          }
        }
      }
    }
    self.widget.event(ctx, event, data, env)
  }

  fn lifecycle(
    &mut self,
    ctx: &mut druid::widget::prelude::LifeCycleCtx,
    event: &druid::widget::prelude::LifeCycle,
    data: &T,
    env: &druid::widget::prelude::Env,
  ) {
    self.widget.lifecycle(ctx, event, data, env);
  }

  fn update(
    &mut self,
    ctx: &mut druid::widget::prelude::UpdateCtx,
    _: &T,
    data: &T,
    env: &druid::widget::prelude::Env,
  ) {
    self.widget.update(ctx, data, env)
  }

  fn layout(
    &mut self,
    ctx: &mut druid::widget::prelude::LayoutCtx,
    bc: &druid::widget::prelude::BoxConstraints,
    data: &T,
    env: &druid::widget::prelude::Env,
  ) -> druid::widget::prelude::Size {
    let unconstrained_size = self.widget.layout(ctx, bc, data, &env);

    if self.last_unconstrained == Some(unconstrained_size.height) && let Some(height) = self.height {
      let child_bc = BoxConstraints::new(
        Size::new(bc.min().width, height),
        Size::new(bc.max().width, height)
      );
      return self.widget.layout(ctx, &child_bc, data, env);
    } else if unconstrained_size.height.is_finite() {
      self.last_unconstrained = Some(unconstrained_size.height);
      let mut linker = self.height_linker.borrow_mut();

      if linker.resolved() {
        linker.reset(ctx)
      } else {
        linker.increment_resolved(ctx, unconstrained_size.height);
      }
    }

    unconstrained_size
  }

  fn paint(
    &mut self,
    ctx: &mut druid::widget::prelude::PaintCtx,
    data: &T,
    env: &druid::widget::prelude::Env,
  ) {
    self.widget.paint(ctx, data, env)
  }
}

use std::{cell::RefCell, rc::Rc};

use druid::{widget::Axis, BoxConstraints, Data, Selector, Size, Widget, WidgetId, WidgetPod};
use druid_widget_nursery::CommandCtx;

pub struct HeightLinker {
  pub linked: usize,
  pub resolved: usize,
  pub max: f64,
  id: WidgetId,
  pub axis: Axis,
}

pub type HeightLinkerShared = Rc<RefCell<HeightLinker>>;

enum HeightLinkerCmd {
  SetHeight(f64),
  ResetHeight,
}

impl HeightLinker {
  const HEIGHT_LINKER_CMD: Selector<(WidgetId, HeightLinkerCmd)> =
    Selector::new("height_linker.command");

  fn new() -> Self {
    Self {
      linked: 0,
      resolved: 0,
      max: f64::NEG_INFINITY,
      id: WidgetId::next(),
      axis: Axis::Vertical,
    }
  }

  pub fn new_shared() -> HeightLinkerShared {
    Rc::new(RefCell::new(Self::new()))
  }

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
    self.resolved = 0;
    self.max = f64::NEG_INFINITY;
    ctx.submit_command(Self::HEIGHT_LINKER_CMD.with((self.id, HeightLinkerCmd::ResetHeight)))
  }
}

pub struct LinkedHeights<T: Data, W: Widget<T>> {
  widget: WidgetPod<T, W>,
  height_linker: HeightLinkerShared,
  constraint: Option<f64>,
  last_unconstrained: Option<f64>,
  axis: Axis,
}

impl<T: Data, W: Widget<T>> LinkedHeights<T, W> {
  pub fn new(widget: W, height_linker: HeightLinkerShared) -> Self {
    let mut borrow = height_linker.borrow_mut();
    borrow.linked += 1;
    let axis = borrow.axis;
    Self {
      widget: WidgetPod::new(widget),
      height_linker: height_linker.clone(),
      constraint: None,
      last_unconstrained: None,
      axis,
    }
  }

  pub fn new_with_linker(widget: W) -> (Self, HeightLinkerShared) {
    let linker = HeightLinker::new_shared();

    let this = Self::new(widget, linker.clone());

    (this, linker)
  }

  pub fn horizontal(mut self) -> Self {
    self.axis = Axis::Horizontal;
    self.height_linker.borrow_mut().axis = Axis::Horizontal;

    self
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
              self.constraint = Some(*height);
              ctx.request_layout()
            }
            HeightLinkerCmd::ResetHeight => {
              self.constraint = None;
              ctx.request_layout()
            }
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
    let unconstrained_size = self.widget.layout(ctx, bc, data, env);

    let unconstrained_value = match self.axis {
      Axis::Horizontal => unconstrained_size.width,
      Axis::Vertical => unconstrained_size.height,
    };

    if self.last_unconstrained == Some(unconstrained_value)
      && let Some(constraint) = self.constraint
    {
      let child_bc = match self.axis {
        Axis::Horizontal => BoxConstraints::tight(Size::new(constraint, unconstrained_size.height)),
        Axis::Vertical => BoxConstraints::tight(Size::new(unconstrained_size.width, constraint)),
      };
      return self.widget.layout(ctx, &child_bc, data, env);
    } else if unconstrained_value.is_finite() {
      self.last_unconstrained = Some(unconstrained_value);
      let mut linker = self.height_linker.borrow_mut();

      if linker.resolved() {
        linker.reset(ctx)
      } else {
        linker.increment_resolved(ctx, unconstrained_value);
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

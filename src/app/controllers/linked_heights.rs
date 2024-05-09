use std::{cell::RefCell, rc::Rc};

use druid::{
  widget::{Axis, WidgetWrapper},
  BoxConstraints, Data, Selector, Size, Widget, WidgetId, WidgetPod,
};
use druid_widget_nursery::CommandCtx;
use proc_macros::Widget;

#[derive(Debug)]
pub struct HeightLinker {
  pub linked: usize,
  pub resolved: usize,
  pub max: f64,
  id: WidgetId,
  pub axis: Axis,
}

impl PartialEq for HeightLinker {
  fn eq(&self, _: &Self) -> bool {
    true
  }
}

impl Eq for HeightLinker {}

pub type HeightLinkerShared = Rc<RefCell<HeightLinker>>;

enum HeightLinkerCmd {
  SetHeight(f64),
  ResetHeight,
}

impl HeightLinker {
  const HEIGHT_LINKER_CMD: Selector<(WidgetId, HeightLinkerCmd)> =
    Selector::new("height_linker.command");
  pub const HEIGHT_LINKER_RESET_ALL: Selector = Selector::new("height_linker.reset.all");

  pub fn new() -> Self {
    Self {
      linked: 0,
      resolved: 0,
      max: f64::NEG_INFINITY,
      id: WidgetId::next(),
      axis: Axis::Vertical,
    }
  }

  pub fn axis(mut self, axis: Axis) -> Self {
    self.axis = axis;
    self
  }

  pub fn new_shared() -> HeightLinkerShared {
    Self::new().shared()
  }

  pub fn shared(self) -> HeightLinkerShared {
    Rc::new(RefCell::new(self))
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
    self.linked > 0 && self.resolved >= self.linked
  }

  fn reset(&mut self, ctx: &mut impl CommandCtx) {
    self.resolved = 0;
    self.max = f64::NEG_INFINITY;
    ctx.submit_command(Self::HEIGHT_LINKER_CMD.with((self.id, HeightLinkerCmd::ResetHeight)))
  }
}

#[derive(Widget)]
#[widget(widget_pod = widget, event = event_impl, layout = layout_impl)]
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

  fn event_impl(
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
      } else if cmd.is(HeightLinker::HEIGHT_LINKER_RESET_ALL) {
        self.constraint = None;
        self.last_unconstrained = None;
        let mut linker = self.height_linker.borrow_mut();
        linker.linked = 0;
        linker.resolved = 0;
        linker.max = f64::NEG_INFINITY;
        ctx.request_layout()
      }
    }
    self.widget.event(ctx, event, data, env)
  }

  fn layout_impl(
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

      let mut size = None;
      if linker.max > unconstrained_value {
        self.constraint = Some(linker.max);
        let child_bc = match self.axis {
          Axis::Horizontal => {
            BoxConstraints::tight(Size::new(linker.max, unconstrained_size.height))
          }
          Axis::Vertical => BoxConstraints::tight(Size::new(unconstrained_size.width, linker.max)),
        };
        size = Some(self.widget.layout(ctx, &child_bc, data, env));
      }
      if linker.resolved() {
        linker.reset(ctx)
      } else {
        linker.increment_resolved(ctx, unconstrained_value);
      }
      if let Some(size) = size {
        return size;
      }
    }

    unconstrained_size
  }
}

impl<T: Data, W: Widget<T>> WidgetWrapper for LinkedHeights<T, W> {
  type Wrapped = W;

  fn wrapped(&self) -> &Self::Wrapped {
    self.widget.widget()
  }

  fn wrapped_mut(&mut self) -> &mut Self::Wrapped {
    self.widget.widget_mut()
  }
}

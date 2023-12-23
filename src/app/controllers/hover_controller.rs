use std::{cell::Cell, rc::Rc};

use druid::{widget::Controller, Cursor, Data, Selector, Widget};

pub type SharedHoverState = Rc<Cell<bool>>;

pub trait HoverState {
  fn set(&mut self, state: bool);
}

impl HoverState for bool {
  fn set(&mut self, state: bool) {
    *self = state
  }
}

impl HoverState for Rc<Cell<bool>> {
  fn set(&mut self, state: bool) {
    self.replace(state);
  }
}

pub struct HoverController<T: HoverState>(pub T);

impl<T: HoverState> HoverController<T> {
  fn set(&mut self, state: bool) {
    self.0.set(state)
  }
}

impl HoverController<bool> {
  pub fn default() -> Self {
    Self(false)
  }
}

const REMOVE_POINTER: Selector = Selector::new("hover_controller.remove_pointer");

impl<T: Data, W: Widget<T>, S: HoverState> Controller<T, W> for HoverController<S> {
  fn event(
    &mut self,
    child: &mut W,
    ctx: &mut druid::EventCtx,
    event: &druid::Event,
    data: &mut T,
    env: &druid::Env,
  ) {
    if let druid::Event::MouseMove(_) = event {
      if !ctx.is_disabled() && (ctx.is_hot() || ctx.is_active()) {
        self.set(true);
        ctx.set_cursor(&Cursor::Pointer);
      } else {
        self.set(false);
        ctx.clear_cursor()
      }
      ctx.request_paint();
    } else if let druid::Event::Command(cmd) = event
      && cmd.is(REMOVE_POINTER)
    {
      self.set(false);
      ctx.clear_cursor()
    }
    child.event(ctx, event, data, env)
  }

  fn lifecycle(
    &mut self,
    child: &mut W,
    ctx: &mut druid::LifeCycleCtx,
    event: &druid::LifeCycle,
    data: &T,
    env: &druid::Env,
  ) {
    if let druid::LifeCycle::HotChanged(false) = event {
      ctx.submit_command(REMOVE_POINTER)
    }

    child.lifecycle(ctx, event, data, env)
  }
}

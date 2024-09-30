use std::{cell::Cell, rc::Rc};

use druid::{widget::Controller, Cursor, Data, Selector, Widget, WidgetId};

use crate::app::util::EventExt;

pub type SharedHoverState = Rc<Cell<bool>>;

pub trait HoverState: Data {
  fn set(&mut self, state: bool);
}

impl HoverState for bool {
  fn set(&mut self, state: bool) {
    *self = state;
  }
}

impl HoverState for Rc<Cell<bool>> {
  fn set(&mut self, state: bool) {
    self.replace(state);
  }
}

#[derive(Clone, Data, Debug)]
pub struct SharedIdHoverState(#[data(ignore)] pub WidgetId, pub Rc<Cell<bool>>);

impl HoverState for SharedIdHoverState {
  fn set(&mut self, state: bool) {
    self.1.replace(state);
  }
}

impl Default for SharedIdHoverState {
  fn default() -> Self {
    Self(WidgetId::next(), Rc::default())
  }
}

pub struct HoverController<T: HoverState = bool> {
  pub state: T,
  dismiss_on_click: bool,
}

impl<T: HoverState> HoverController<T> {
  pub fn new(state: T, dismiss_on_click: bool) -> Self {
    Self {
      state,
      dismiss_on_click,
    }
  }

  fn set(&mut self, state: bool) {
    self.state.set(state);
  }
}

impl Default for HoverController<bool> {
  fn default() -> Self {
    Self {
      state: false,
      dismiss_on_click: false,
    }
  }
}

pub const REMOVE_POINTER: Selector = Selector::new("hover_controller.remove_pointer");

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
        ctx.clear_cursor();
      }
      ctx.request_paint();
    } else if (self.dismiss_on_click && event.as_mouse_up().is_some())
      || event.is_cmd(REMOVE_POINTER)
    {
      self.set(false);
      ctx.clear_cursor();
      ctx.request_update();
      ctx.request_paint();
    }
    child.event(ctx, event, data, env);
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
      ctx.submit_command(REMOVE_POINTER);
    }

    child.lifecycle(ctx, event, data, env);
  }
}

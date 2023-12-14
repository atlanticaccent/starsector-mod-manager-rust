use druid::{widget::Controller, Cursor, Data, Widget, Selector};

pub struct HoverController;

const REMOVE_POINTER: Selector = Selector::new("hover_controller.remove_pointer");

impl<T: Data, W: Widget<T>> Controller<T, W> for HoverController {
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
        ctx.set_cursor(&Cursor::Pointer);
      } else {
        ctx.clear_cursor()
      }
      ctx.request_paint();
    } else if let druid::Event::Command(cmd) = event && cmd.is(REMOVE_POINTER) {
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

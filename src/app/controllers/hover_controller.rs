use druid::{widget::Controller, Cursor, Data, Widget};

pub struct HoverController;

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
      }
      ctx.request_paint();
    }
    child.event(ctx, event, data, env)
  }
}

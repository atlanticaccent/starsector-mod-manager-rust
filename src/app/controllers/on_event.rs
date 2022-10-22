use druid::widget::prelude::*;
use druid::widget::Controller;

pub struct OnEvent<T> {
  handler: Box<dyn Fn(&mut EventCtx, &Event, &mut T) -> bool>,
}

#[allow(dead_code)]
impl<T> OnEvent<T> {
  pub fn new(handler: impl Fn(&mut EventCtx, &Event, &mut T) -> bool + 'static) -> Self {
    Self {
      handler: Box::new(handler),
    }
  }
}

impl<T: Data, W: Widget<T>> Controller<T, W> for OnEvent<T> {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
    if (self.handler)(ctx, event, data) {
      ctx.set_handled();
    }
    child.event(ctx, event, data, env);
  }
}

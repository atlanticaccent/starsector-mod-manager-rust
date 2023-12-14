use druid::widget::prelude::*;
use druid::widget::Controller;

pub struct OnHover<T, W: Widget<T>> {
  handler: Box<dyn Fn(&mut W, &mut EventCtx, &mut T) -> bool>,
}

#[allow(dead_code)]
impl<T, W: Widget<T>> OnHover<T, W> {
  pub fn new(handler: impl Fn(&mut W, &mut EventCtx, &mut T) -> bool + 'static) -> Self {
    Self {
      handler: Box::new(handler),
    }
  }
}

impl<T: Data, W: Widget<T>> Controller<T, W> for OnHover<T, W> {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
    if ctx.is_hot() {
      if (self.handler)(child, ctx, data) {
        ctx.set_handled();
      }
    }
    child.event(ctx, event, data, env);
  }
}

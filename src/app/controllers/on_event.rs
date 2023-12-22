use druid::widget::{prelude::*, Controller};

pub struct OnEvent<T, W: Widget<T>> {
  handler: Box<dyn Fn(&mut W, &mut EventCtx, &Event, &mut T) -> bool>,
}

#[allow(dead_code)]
impl<T, W: Widget<T>> OnEvent<T, W> {
  pub fn new(handler: impl Fn(&mut W, &mut EventCtx, &Event, &mut T) -> bool + 'static) -> Self {
    Self {
      handler: Box::new(handler),
    }
  }
}

impl<T: Data, W: Widget<T>> Controller<T, W> for OnEvent<T, W> {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
    if (self.handler)(child, ctx, event, data) {
      ctx.set_handled();
    }
    child.event(ctx, event, data, env);
  }
}

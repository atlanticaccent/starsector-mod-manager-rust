use std::marker::PhantomData;

use druid::widget::{prelude::*, Controller};

pub struct OnEvent<T, W: Widget<T>, F: Fn(&mut W, &mut EventCtx, &Event, &mut T) -> bool + 'static>
{
  handler: F,
  data: PhantomData<T>,
  widget: PhantomData<W>,
}

pub type BoxedOnEvent<T, W> =
  OnEvent<T, W, Box<dyn Fn(&mut W, &mut EventCtx, &Event, &mut T) -> bool>>;

#[allow(dead_code)]
impl<T, W: Widget<T>, F: Fn(&mut W, &mut EventCtx, &Event, &mut T) -> bool + 'static>
  OnEvent<T, W, F>
{
  pub fn new(handler: F) -> Self {
    Self {
      handler,
      data: PhantomData,
      widget: PhantomData,
    }
  }
}

impl<T: Data, W: Widget<T>, F: Fn(&mut W, &mut EventCtx, &Event, &mut T) -> bool + 'static>
  Controller<T, W> for OnEvent<T, W, F>
{
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut T, env: &Env) {
    if (self.handler)(child, ctx, event, data) {
      ctx.set_handled();
    }
    child.event(ctx, event, data, env);
  }
}

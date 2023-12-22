use druid::{
  widget::{prelude::*, Controller},
  Selector,
};

type HandlerFn<CT, WT, W> = Box<dyn Fn(&mut W, &mut EventCtx, &CT, &mut WT) -> bool>;

pub struct OnCmd<CT, WT, W: Widget<WT>> {
  selector: Selector<CT>,
  handler: HandlerFn<CT, WT, W>,
}

impl<CT, WT, W: Widget<WT>> OnCmd<CT, WT, W> {
  pub fn new(
    selector: Selector<CT>,
    handler: impl Fn(&mut W, &mut EventCtx, &CT, &mut WT) -> bool + 'static,
  ) -> Self {
    Self {
      selector,
      handler: Box::new(handler),
    }
  }
}

impl<WT: Data, W: Widget<WT>, CT: 'static> Controller<WT, W> for OnCmd<CT, WT, W> {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut WT, env: &Env) {
    match event {
      Event::Command(c) if c.is(self.selector) => {
        if !(self.handler)(child, ctx, c.get_unchecked(self.selector), data) {
          return;
        }
      }
      _ => {}
    }
    child.event(ctx, event, data, env);
  }
}

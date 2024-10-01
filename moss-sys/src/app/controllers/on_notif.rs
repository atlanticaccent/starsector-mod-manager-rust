use std::marker::PhantomData;

use druid::{
  widget::{prelude::*, Controller},
  Selector,
};

pub struct OnNotif<CT, WT, F: Fn(&mut EventCtx, &CT, &mut WT)> {
  selector: Selector<CT>,
  handler: F,
  _data: PhantomData<WT>,
}

impl<CT, WT, F: Fn(&mut EventCtx, &CT, &mut WT) + 'static> OnNotif<CT, WT, F> {
  pub fn new(selector: Selector<CT>, handler: F) -> Self {
    Self {
      selector,
      handler,
      _data: PhantomData,
    }
  }
}

impl<WT: Data, W: Widget<WT>, CT: 'static, F: Fn(&mut EventCtx, &CT, &mut WT)> Controller<WT, W>
  for OnNotif<CT, WT, F>
{
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut WT, env: &Env) {
    match event {
      Event::Notification(notif) if notif.is(self.selector) => {
        (self.handler)(ctx, notif.get(self.selector).unwrap(), data);
        ctx.set_handled();
      }
      _ => {}
    }
    child.event(ctx, event, data, env);
  }
}

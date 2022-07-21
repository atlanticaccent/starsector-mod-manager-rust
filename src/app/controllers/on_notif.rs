use druid::widget::prelude::*;
use druid::widget::Controller;
use druid::Selector;

pub struct OnNotif<CT, WT> {
  selector: Selector<CT>,
  handler: Box<dyn Fn(&mut EventCtx, &CT, &mut WT)>,
}

impl<CT, WT> OnNotif<CT, WT> {
  pub fn new(
    selector: Selector<CT>,
    handler: impl Fn(&mut EventCtx, &CT, &mut WT) + 'static,
  ) -> Self {
    Self {
      selector,
      handler: Box::new(handler),
    }
  }
}

impl<WT: Data, W: Widget<WT>, CT: 'static> Controller<WT, W> for OnNotif<CT, WT> {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut WT, env: &Env) {
    match event {
      Event::Notification(notif) if notif.is(self.selector) => {
        (self.handler)(ctx, notif.get(self.selector).unwrap(), data);
        ctx.set_handled()
      }
      _ => {}
    }
    child.event(ctx, event, data, env);
  }
}

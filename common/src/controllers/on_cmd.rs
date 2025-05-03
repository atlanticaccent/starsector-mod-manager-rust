use druid::{
  widget::{prelude::*, Controller},
  Selector,
};

type HandlerFn<CT, WT, W> = Box<dyn Fn(&mut W, &mut EventCtx, &CT, &mut WT) -> bool>;
type HandlerFnWithEnv<CT, WT, W> = Box<dyn Fn(&mut W, &mut EventCtx, &CT, &mut WT, &Env) -> bool>;

pub enum CommandFn<CT, WT, W> {
  Plain(HandlerFn<CT, WT, W>),
  WithEnv(HandlerFnWithEnv<CT, WT, W>),
}

pub struct OnCmd<CT, WT, W: Widget<WT>> {
  selector: Selector<CT>,
  handler: CommandFn<CT, WT, W>,
}

impl<CT, WT, W: Widget<WT>> OnCmd<CT, WT, W> {
  pub fn new(selector: Selector<CT>, handler: CommandFn<CT, WT, W>) -> Self {
    Self { selector, handler }
  }
}

impl<WT: Data, W: Widget<WT>, CT: 'static> Controller<WT, W> for OnCmd<CT, WT, W> {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut WT, env: &Env) {
    match event {
      Event::Command(c) if c.is(self.selector) => {
        let res = match &self.handler {
          CommandFn::Plain(f) => f(child, ctx, c.get_unchecked(self.selector), data),
          CommandFn::WithEnv(f) => f(child, ctx, c.get_unchecked(self.selector), data, env),
        };
        if !res {
          return;
        }
      }
      _ => {}
    }
    child.event(ctx, event, data, env);
  }
}

use druid::{
  widget::Controller, Command, Data, Env, Event, EventCtx, LifeCycle, LifeCycleCtx, Selector,
  Widget,
};

pub type OnChange<T, W> = Box<dyn Fn(&mut W, &mut EventCtx, &T, &mut T, &Env) -> bool>;
pub type OnCmd<T, W> = Box<dyn Fn(&mut W, &mut EventCtx, &Command, &mut T) -> bool>;
pub type OnEvent<T, W> = Box<dyn Fn(&mut W, &mut EventCtx, &Event, &mut T) -> bool>;
pub type OnAdded<T, W> = Box<dyn Fn(&mut W, &mut LifeCycleCtx, &T, &Env)>;
pub type OnLifecycle<T, W> = Box<dyn Fn(&mut W, &mut LifeCycleCtx, &LifeCycle, &T, &Env)>;

pub struct ExtensibleController<T, W> {
  on_change: Vec<OnChange<T, W>>,
  on_command: Vec<OnCmd<T, W>>,
  on_event: Vec<OnEvent<T, W>>,
  on_added: Vec<OnAdded<T, W>>,
  on_lifecycle: Vec<OnLifecycle<T, W>>,
}

impl<T, W: Widget<T>> ExtensibleController<T, W> {
  pub fn new() -> Self {
    Self {
      on_change: Vec::new(),
      on_command: Vec::new(),
      on_event: Vec::new(),
      on_added: Vec::new(),
      on_lifecycle: Vec::new(),
    }
  }

  pub fn on_change(
    mut self,
    handler: impl Fn(&mut W, &mut EventCtx, &T, &mut T, &Env) -> bool + 'static,
  ) -> Self {
    self.on_change.push(Box::new(handler));

    self
  }

  pub fn on_command<P: 'static>(
    mut self,
    selector: Selector<P>,
    handler: impl Fn(&mut W, &mut EventCtx, &P, &mut T) -> bool + 'static,
  ) -> Self {
    self.on_command.push(Box::new(
      move |child: &mut W, ctx: &mut EventCtx, cmd: &Command, data: &mut T| {
        if let Some(payload) = cmd.get(selector) {
          handler(child, ctx, payload, data)
        } else {
          true
        }
      },
    ));

    self
  }

  pub fn on_added(
    mut self,
    handler: impl Fn(&mut W, &mut LifeCycleCtx, &T, &Env) + 'static,
  ) -> Self {
    self.on_added.push(Box::new(handler));

    self
  }

  pub fn on_lifecycle(
    mut self,
    handler: impl Fn(&mut W, &mut LifeCycleCtx, &LifeCycle, &T, &Env) + 'static,
  ) -> Self {
    self.on_lifecycle.push(Box::new(handler));

    self
  }

  pub fn on_event(
    mut self,
    handler: impl (Fn(&mut W, &mut EventCtx, &Event, &mut T) -> bool) + 'static,
  ) -> Self {
    self.on_event.push(Box::new(handler));

    self
  }
}

impl<T: Data, W: Widget<T>> Controller<T, W> for ExtensibleController<T, W> {
  fn event(
    &mut self,
    child: &mut W,
    ctx: &mut EventCtx,
    event: &druid::Event,
    data: &mut T,
    env: &Env,
  ) {
    let old_data = data.clone();

    if let druid::Event::Command(cmd) = event {
      let mut exit = false;
      for handler in &mut self.on_command {
        exit |= !handler(child, ctx, cmd, data);
      }
      if exit {
        return;
      }
    } else {
      let mut exit = false;
      for handler in &mut self.on_event {
        exit |= !handler(child, ctx, event, data);
      }
      if exit {
        return;
      }
    }

    child.event(ctx, event, data, env);

    if !old_data.same(data) {
      for handler in &mut self.on_change {
        if !handler(child, ctx, &old_data, data, env) {
          return;
        }
      }
    }
  }

  fn lifecycle(
    &mut self,
    child: &mut W,
    ctx: &mut druid::LifeCycleCtx,
    event: &druid::LifeCycle,
    data: &T,
    env: &Env,
  ) {
    for handler in &mut self.on_lifecycle {
      handler(child, ctx, event, data, env)
    }
    if let druid::LifeCycle::WidgetAdded = event {
      for handler in &mut self.on_added {
        handler(child, ctx, data, env);
      }
    }

    child.lifecycle(ctx, event, data, env)
  }
}

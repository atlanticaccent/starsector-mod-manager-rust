use std::{cell::RefCell, rc::Rc};

use druid::{Data, Widget, WidgetPod};

pub struct DelayedPainter<T, W> {
  widget: Rc<RefCell<WidgetPod<T, W>>>,
  z_index: u32,
}

impl<T, W: Widget<T>> DelayedPainter<T, W> {
  pub fn new(widget: W, z_index: u32) -> Self {
    Self {
      widget: Rc::new(RefCell::new(WidgetPod::new(widget))),
      z_index,
    }
  }
}

impl<T: Data, W: Widget<T> + 'static> Widget<T> for DelayedPainter<T, W> {
  fn event(
    &mut self,
    ctx: &mut druid::widget::prelude::EventCtx,
    event: &druid::widget::prelude::Event,
    data: &mut T,
    env: &druid::widget::prelude::Env,
  ) {
    self.widget.borrow_mut().event(ctx, event, data, env);
  }

  fn lifecycle(
    &mut self,
    ctx: &mut druid::widget::prelude::LifeCycleCtx,
    event: &druid::widget::prelude::LifeCycle,
    data: &T,
    env: &druid::widget::prelude::Env,
  ) {
    self.widget.borrow_mut().lifecycle(ctx, event, data, env);
  }

  fn update(
    &mut self,
    ctx: &mut druid::widget::prelude::UpdateCtx,
    _old_data: &T,
    data: &T,
    env: &druid::widget::prelude::Env,
  ) {
    self.widget.borrow_mut().update(ctx, data, env);
  }

  fn layout(
    &mut self,
    ctx: &mut druid::widget::prelude::LayoutCtx,
    bc: &druid::widget::prelude::BoxConstraints,
    data: &T,
    env: &druid::widget::prelude::Env,
  ) -> druid::widget::prelude::Size {
    self.widget.borrow_mut().layout(ctx, bc, data, env)
  }

  fn paint(
    &mut self,
    ctx: &mut druid::widget::prelude::PaintCtx,
    data: &T,
    env: &druid::widget::prelude::Env,
  ) {
    let widget = self.widget.clone();
    let data = data.clone();
    let env = env.clone();
    ctx.paint_with_z_index(self.z_index, move |ctx| {
      widget.borrow_mut().paint(ctx, &data, &env);
    });
  }
}

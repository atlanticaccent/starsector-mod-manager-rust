use druid::{widget::Painter, Data, Widget};

pub struct UnconditionalPainter<T>(pub Painter<T>);

impl<T> UnconditionalPainter<T> {
  pub fn new(f: impl FnMut(&mut druid::PaintCtx, &T, &druid::Env) + 'static) -> Self {
    Self(Painter::new(f))
  }
}

impl<T: Data> Widget<T> for UnconditionalPainter<T> {
  fn event(
    &mut self,
    _ctx: &mut druid::EventCtx,
    _event: &druid::Event,
    _data: &mut T,
    _env: &druid::Env,
  ) {
  }

  fn lifecycle(
    &mut self,
    _ctx: &mut druid::LifeCycleCtx,
    _event: &druid::LifeCycle,
    _data: &T,
    _env: &druid::Env,
  ) {
  }

  fn update(&mut self, _ctx: &mut druid::UpdateCtx, _old_data: &T, _data: &T, _env: &druid::Env) {}

  fn layout(
    &mut self,
    ctx: &mut druid::LayoutCtx,
    bc: &druid::BoxConstraints,
    data: &T,
    env: &druid::Env,
  ) -> druid::Size {
    self.0.layout(ctx, bc, data, env)
  }

  fn paint(&mut self, ctx: &mut druid::PaintCtx, data: &T, env: &druid::Env) {
    self.0.paint(ctx, data, env)
  }
}

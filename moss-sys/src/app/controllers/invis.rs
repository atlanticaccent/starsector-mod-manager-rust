use druid::{Data, Env, PaintCtx, UpdateCtx, Widget, WidgetPod};
use proc_macros::Widget;

type InvisibleFn<T> = Box<dyn Fn(&T, &Env) -> bool>;

#[derive(Widget)]
#[widget(widget_pod = 1, paint = paint_impl, update = update_impl)]
pub struct InvisibleIf<T, W>(InvisibleFn<T>, WidgetPod<T, W>);

impl<T: Data, W: Widget<T>> InvisibleIf<T, W> {
  pub fn new(test: impl Fn(&T, &Env) -> bool + 'static, widget: W) -> Self {
    Self(Box::new(test), WidgetPod::new(widget))
  }

  fn paint_impl(&mut self, ctx: &mut PaintCtx, data: &T, env: &Env) {
    if !self.0(data, env) {
      self.1.paint(ctx, data, env);
    }
  }

  fn update_impl(&mut self, ctx: &mut UpdateCtx, old_data: &T, data: &T, env: &Env) {
    if !old_data.same(data) {
      ctx.request_paint();
    }
    self.1.update(ctx, data, env);
  }
}

use druid::{widget::Axis, BoxConstraints, Data, Env, LayoutCtx, Size, Widget, WidgetPod};
use proc_macros::Widget;

#[derive(Widget)]
#[widget(layout = layout_impl)]
pub struct MaxSizeBox<T, W> {
  widget_pod: WidgetPod<T, W>,
  axis: Axis,
  max: f64,
}

impl<T: Data, W: Widget<T>> MaxSizeBox<T, W> {
  pub fn new(widget: W, axis: Axis, max: f64) -> Self {
    Self {
      widget_pod: WidgetPod::new(widget),
      axis,
      max,
    }
  }

  fn layout_impl(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
    let constraint_max = match self.axis {
      Axis::Horizontal => bc.max().width,
      Axis::Vertical => bc.max().height,
    };

    if constraint_max > self.max {
      self
        .widget_pod
        .layout(ctx, &bc.shrink_max_to(self.axis, self.max), data, env)
    } else {
      self.widget_pod.layout(ctx, bc, data, env)
    }
  }
}

use druid::{WidgetPod, Widget, Data, Rect, Size, widget::Axis};

pub struct MatchDimensions<T, W>
where
  T: Data,
  W: Widget<T>
{
  children: Vec<WidgetPod<T, W>>,
  axis: Axis
}

impl<T: Data, W: Widget<T>> Widget<T> for MatchDimensions<T, W> {
  fn event(&mut self, ctx: &mut druid::EventCtx, event: &druid::Event, data: &mut T, env: &druid::Env) {
    for child in self.children.iter_mut() {
      child.event(ctx, event, data, env)
    }
  }
  
  fn lifecycle(&mut self, ctx: &mut druid::LifeCycleCtx, event: &druid::LifeCycle, data: &T, env: &druid::Env) {
    for child in self.children.iter_mut() {
      child.lifecycle(ctx, event, data, env)
    }
  }
  
  fn update(&mut self, ctx: &mut druid::UpdateCtx, old_data: &T, data: &T, env: &druid::Env) {
    for child in self.children.iter_mut() {
      child.update(ctx, data, env)
    }
  }
  
  fn layout(&mut self, ctx: &mut druid::LayoutCtx, bc: &druid::BoxConstraints, data: &T, env: &druid::Env) -> druid::Size {
    let unbounded_child_rects: Vec<Size> = self.children.iter_mut().map(|child| {
      child.layout(ctx, bc, data, env)
    })
    .collect();

    let max_dim = unbounded_child_rects.iter().map(|s| {
      match self.axis {
        Axis::Vertical => {
          s.width
        },
        Axis::Horizontal => {
          s.height
        }
      }
    })
    .reduce(f64::max)
    .unwrap_or_default();

    
  }
  
  fn paint(&mut self, ctx: &mut druid::PaintCtx, data: &T, env: &druid::Env) {
    for child in self.children.iter_mut() {
      child.paint(ctx, data, env)
    }
  }
}

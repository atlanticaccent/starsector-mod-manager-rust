use druid::{EventCtx, Widget};

use crate::app::controllers::{BoxedOnEvent, OnEvent};

pub struct OnHover;

#[allow(dead_code)]
impl OnHover {
  pub fn new<T, W: Widget<T>>(
    handler: impl Fn(&mut W, &mut EventCtx, &mut T) -> bool + 'static,
  ) -> BoxedOnEvent<T, W> {
    OnEvent::new(Box::new(move |widget, ctx, _, data| {
      handler(widget, ctx, data)
    }))
  }
}

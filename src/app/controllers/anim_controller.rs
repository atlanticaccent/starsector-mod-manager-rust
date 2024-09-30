use druid::{widget::Controller, Data, Selector, Widget};
use druid_widget_nursery::animation::{AnimationController, AnimationCurve, Interpolate};

pub struct AnimController<T> {
  animator: AnimationController,
  curve: AnimationCurve,
  start: T,
  end: T,
  transform: Option<Box<dyn Fn(T) -> T>>,
  start_on_added: bool,
}

impl<T: Interpolate> AnimController<T> {
  pub const ANIM_START: Selector = Selector::new("anim_controller.start");

  pub fn new(start: T, end: T, curve: AnimationCurve) -> Self {
    Self {
      animator: AnimationController::new(),
      start,
      end,
      curve,
      transform: None,
      start_on_added: false,
    }
  }

  pub fn with_transform(mut self, transform: impl Fn(T) -> T + 'static) -> Self {
    self.transform = Some(Box::new(transform));
    self
  }

  pub fn with_duration(mut self, duration: f64) -> Self {
    self.animator.set_duration(duration);
    self
  }

  pub fn looping(mut self) -> Self {
    self.animator.set_repeat_limit(None);
    self
  }
}

impl<T: Data + Interpolate, W: Widget<T>> Controller<T, W> for AnimController<T> {
  fn event(
    &mut self,
    child: &mut W,
    ctx: &mut druid::EventCtx,
    event: &druid::Event,
    data: &mut T,
    env: &druid::Env,
  ) {
    if !self.start_on_added
      && let druid::Event::Command(cmd) = &event
      && cmd.is(Self::ANIM_START)
    {
      self.animator.start(ctx);
    }

    if let druid::Event::AnimFrame(nanos) = &event {
      self.animator.update(ctx, *nanos);
    }
    if self.animator.animating() {
      let fraction = self.animator.fraction();
      let mut inter_data = self
        .start
        .interpolate(&self.end, self.curve.translate(fraction));
      if let Some(transform) = self.transform.as_ref() {
        inter_data = transform(inter_data);
      }
      child.event(ctx, event, &mut inter_data, env);
      *data = inter_data;
    } else {
      child.event(ctx, event, data, env);
    }
  }

  fn lifecycle(
    &mut self,
    child: &mut W,
    ctx: &mut druid::LifeCycleCtx,
    event: &druid::LifeCycle,
    data: &T,
    env: &druid::Env,
  ) {
    if let druid::LifeCycle::WidgetAdded = &event {
      if self.start_on_added {
        self.animator.start(ctx);
      }
    }
    child.lifecycle(ctx, event, data, env);
  }

  fn update(
    &mut self,
    child: &mut W,
    ctx: &mut druid::UpdateCtx,
    old_data: &T,
    data: &T,
    env: &druid::Env,
  ) {
    child.update(ctx, old_data, data, env);
  }
}

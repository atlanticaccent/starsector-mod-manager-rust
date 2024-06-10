use std::{
  any::{type_name, Any},
  cell::RefCell,
  collections::HashMap,
};

use druid::{Data, Env, Widget, WidgetPod};
use xxhash_rust::xxh3::Xxh3Builder;

thread_local! {
  static WIDGET_MAP: RefCell<HashMap<usize, Box<dyn Any>, Xxh3Builder>> = const { RefCell::new(HashMap::with_hasher(Xxh3Builder::new())) }
}

pub struct EcsWidget<T, W: Widget<T>> {
  key: Key<T>,
  constructor: Box<dyn Fn() -> W>,
}

pub enum Key<T> {
  Fixed(Option<usize>),
  Dynamic(Box<dyn Fn(&T, &Env) -> Option<usize>>),
}

impl<T> From<usize> for Key<T> {
  fn from(value: usize) -> Self {
    Self::Fixed(Some(value))
  }
}

impl<T> From<Option<usize>> for Key<T> {
  fn from(value: Option<usize>) -> Self {
    Self::Fixed(value)
  }
}

impl<T, F: Fn(&T, &Env) -> Option<usize> + 'static> From<F> for Key<T> {
  fn from(value: F) -> Self {
    Self::Dynamic(Box::new(value))
  }
}

impl<T> Key<T> {
  fn resolve(&self, data: &T, env: &Env) -> Option<usize> {
    match self {
      Key::Fixed(idx) => *idx,
      Key::Dynamic(dynamic) => dynamic(data, env),
    }
  }
}

impl<T: 'static, W: Widget<T> + 'static> EcsWidget<T, W> {
  pub fn new(key: impl Into<Key<T>>, constructor: impl Fn() -> W + 'static) -> Self {
    Self {
      key: key.into(),
      constructor: Box::new(constructor),
    }
  }

  fn apply<U: Default>(
    &self,
    data: &T,
    env: &Env,
    func: impl FnMut(&mut WidgetPod<T, W>, &T, &Env) -> U,
  ) -> U {
    let key = self.key.resolve(data, env);
    Self::apply_inner(key, || (self.constructor)(), data, env, func)
  }

  fn apply_inner<U: Default>(
    key: impl Into<Key<T>>,
    constructor: impl Fn() -> W,
    data: &T,
    env: &Env,
    mut func: impl FnMut(&mut WidgetPod<T, W>, &T, &Env) -> U,
  ) -> U {
    if let Some(key) = key.into().resolve(data, env) {
      WIDGET_MAP.with_borrow_mut(|map| {
        if !map.contains_key(&key) {
          map.insert(key, Box::new(WidgetPod::new((constructor)())));
        }
        let any = map.get_mut(&key).expect(&format!("Get widget at {key}"));

        let widget = any
          .downcast_mut::<WidgetPod<T, W>>()
          .expect(&format!("Cast to widget type {}", type_name::<W>()));
        func(widget, data, env)
      })
    } else {
      U::default()
    }
  }

  fn apply_mut<U: Default>(
    &self,
    data: &mut T,
    env: &Env,
    mut func: impl FnMut(&mut WidgetPod<T, W>, &mut T, &Env) -> U,
  ) -> U {
    if let Some(key) = self.key.resolve(data, env) {
      WIDGET_MAP.with_borrow_mut(|map| {
        if !map.contains_key(&key) {
          map.insert(key, Box::new(WidgetPod::new((self.constructor)())));
        }
        let any = map.get_mut(&key).expect(&format!("Get widget at {key}"));
        let widget = any
          .downcast_mut::<WidgetPod<T, W>>()
          .expect(&format!("Cast to widget type {}", type_name::<W>()));
        func(widget, data, env)
      })
    } else {
      U::default()
    }
  }

  pub fn is_initialized(&self, data: &T, env: &Env) -> bool {
    let key = self.key.resolve(data, env);
    WIDGET_MAP.with_borrow(|map| {
      key
        .and_then(|key| map.get(&key))
        .and_then(|any| any.downcast_ref::<WidgetPod<T, W>>())
        .map(|w| w.is_initialized())
        .unwrap_or_default()
    })
  }
}

impl<T: Data, W: Widget<T> + 'static> Widget<T> for EcsWidget<T, W> {
  fn event(&mut self, ctx: &mut druid::EventCtx, event: &druid::Event, data: &mut T, env: &Env) {
    self.apply_mut(data, env, |widget, data, env| {
      widget.event(ctx, event, data, env)
    })
  }

  fn lifecycle(
    &mut self,
    ctx: &mut druid::LifeCycleCtx,
    event: &druid::LifeCycle,
    data: &T,
    env: &Env,
  ) {
    self.apply(data, env, |widget, data, env| {
      if let druid::LifeCycle::WidgetAdded = event
        && widget.is_initialized()
      {
        return;
      }
      widget.lifecycle(ctx, event, data, env)
    })
  }

  fn update(&mut self, ctx: &mut druid::UpdateCtx, _old_data: &T, data: &T, env: &Env) {
    self.apply(data, env, |widget, data, env| widget.update(ctx, data, env))
  }

  fn layout(
    &mut self,
    ctx: &mut druid::LayoutCtx,
    bc: &druid::BoxConstraints,
    data: &T,
    env: &Env,
  ) -> druid::Size {
    self.apply(data, env, |widget, data, env| {
      widget.layout(ctx, bc, data, env)
    })
  }

  fn paint(&mut self, ctx: &mut druid::PaintCtx, data: &T, env: &Env) {
    self.apply(data, env, |widget, data, env| widget.paint(ctx, data, env))
  }
}

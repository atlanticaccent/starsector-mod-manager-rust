use std::{
  any::{type_name, Any},
  cell::RefCell,
  collections::HashMap,
};

use druid::{Data, Env, Widget};
use xxhash_rust::xxh3::Xxh3Builder;

thread_local! {
  static WIDGET_MAP: RefCell<HashMap<usize, Box<dyn Any>, Xxh3Builder>> = const { RefCell::new(HashMap::with_hasher(Xxh3Builder::new())) }
}

pub struct EcsWidget<T, W: Widget<T>> {
  key: Key<T>,
  constructor: Box<dyn Fn() -> W>,
}

pub enum Key<T> {
  Fixed(usize),
  Dynamic(Box<dyn Fn(&T, &Env) -> usize>),
}

impl<T> From<usize> for Key<T> {
  fn from(value: usize) -> Self {
    Self::Fixed(value)
  }
}

impl<T, F: Fn(&T, &Env) -> usize + 'static> From<F> for Key<T> {
  fn from(value: F) -> Self {
    Self::Dynamic(Box::new(value))
  }
}

impl<T> Key<T> {
  fn resolve(&self, data: &T, env: &Env) -> usize {
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

  fn apply<U>(&self, data: &T, env: &Env, mut func: impl FnMut(&mut W, &T, &Env) -> U) -> U {
    let key = self.key.resolve(data, env);
    WIDGET_MAP.with_borrow_mut(|map| {
      if !map.contains_key(&key) {
        map.insert(key, Box::new((self.constructor)()));
      }
      let any = map.get_mut(&key).expect(&format!("Get widget at {key}"));

      let widget = any
        .downcast_mut::<W>()
        .expect(&format!("Cast to widget type {}", type_name::<W>()));
      func(widget, data, env)
    })
  }

  fn apply_with_key<U>(
    key: impl Into<Key<T>>,
    data: &T,
    env: &Env,
    mut func: impl FnMut(&mut W, &T, &Env) -> U,
  ) -> U {
    let key = key.into().resolve(data, env);
    WIDGET_MAP.with_borrow_mut(|map| {
      let any = map.get_mut(&key).expect(&format!("Get widget at {key}"));

      let widget = any
        .downcast_mut::<W>()
        .expect(&format!("Cast to widget type {}", type_name::<W>()));
      func(widget, data, env)
    })
  }

  fn apply_mut<U>(
    &self,
    data: &mut T,
    env: &Env,
    mut func: impl FnMut(&mut W, &mut T, &Env) -> U,
  ) -> U {
    let key = self.key.resolve(data, env);
    WIDGET_MAP.with_borrow_mut(|map| {
      if !map.contains_key(&key) {
        map.insert(key, Box::new((self.constructor)()));
      }
      let any = map.get_mut(&key).expect(&format!("Get widget at {key}"));
      let widget = any
        .downcast_mut::<W>()
        .expect(&format!("Cast to widget type {}", type_name::<W>()));
      func(widget, data, env)
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
      widget.lifecycle(ctx, event, data, env)
    })
  }

  fn update(&mut self, ctx: &mut druid::UpdateCtx, old_data: &T, data: &T, env: &Env) {
    self.apply(data, env, |widget, data, env| {
      widget.update(ctx, old_data, data, env)
    })
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
    let key = self.key.resolve(data, env);
    let data = data.clone();
    let env = env.clone();
    ctx.paint_with_z_index(1_000_000, move |ctx| {
      Self::apply_with_key(key, &data, &env, |widget, data, env| {
        widget.paint(ctx, data, env)
      })
    });
  }
}

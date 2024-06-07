use std::{cell::RefCell, collections::BTreeMap};

use druid::{widget::Axis, BoxConstraints, Data, Env, Key, LayoutCtx, Size, Widget, WidgetPod};
use proc_macros::Widget;

type ConstraintMap = BTreeMap<(u64, u64), SharedConstraintState>;

thread_local! {
  static CONSTRAINT_MAP: RefCell<ConstraintMap> = const { RefCell::new(BTreeMap::new()) };
  static COUNTER: RefCell<u64> = const { RefCell::new(0) };
}

pub const PARENT_LAYOUT_REPEATER_ID: Key<u64> = Key::new("layout_repeater.id");

#[derive(Debug, Clone, PartialEq)]
pub struct SharedConstraintState {
  pub axis: Axis,
  pub values: Vec<f64>,
  pub constraint: Option<f64>,
}

impl Default for SharedConstraintState {
  fn default() -> Self {
    Self {
      axis: Axis::Vertical,
      values: Default::default(),
      constraint: Default::default(),
    }
  }
}

#[derive(Widget)]
#[widget(widget_pod = child, layout = layout_impl)]
pub struct LayoutRepeater<T, W> {
  id: u64,
  child: WidgetPod<T, W>,
}

impl<T: Data, W: Widget<T>> LayoutRepeater<T, W> {
  pub fn new(id: u64, child: W) -> Self {
    Self {
      id,
      child: WidgetPod::new(child),
    }
  }

  fn layout_impl(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
    self.clear_or_init();
    let mut cached = CONSTRAINT_MAP.with_borrow(Clone::clone);

    let env = env.clone().adding(PARENT_LAYOUT_REPEATER_ID, self.id);
    let size = self.child.layout(ctx, bc, data, &env);

    let recalc = CONSTRAINT_MAP.with_borrow_mut(|map| {
      let diff = diff_maps(&mut cached, map);

      let mut recalc = false;
      diff.for_each(|data| {
        recalc = true;
        data.constraint = data.values.iter().cloned().reduce(f64::max)
      });

      recalc
    });

    if recalc {
      self.child.layout(ctx, bc, data, &env)
    } else {
      size
    }
  }

  fn clear_or_init(&self) {
    CONSTRAINT_MAP.with_borrow_mut(|map| {
      for (_, data) in map.range_mut((self.id, 0)..(self.id + 1, 0)) {
        data.values.clear();
        data.constraint = None;
      }
    });
  }
}

pub fn next_id() -> u64 {
  COUNTER.with_borrow_mut(|count| {
    let val = *count;
    *count += 1;
    val
  })
}

fn diff_maps<'a>(
  old: &'a mut ConstraintMap,
  new: &'a mut ConstraintMap,
) -> impl Iterator<Item = &'a mut SharedConstraintState> {
  new
    .iter_mut()
    .filter_map(move |(key, value)| (Some(&value) != old.get_mut(key).as_ref()).then_some(value))
}

#[derive(Widget)]
#[widget(widget_pod = child, layout = layout_impl)]
pub struct SharedConstraint<T, W> {
  child: WidgetPod<T, W>,
  shared_id: ConstraintId<T>,
  axis: Axis,
}

impl<T: Data, W: Widget<T>> SharedConstraint<T, W> {
  pub fn new(widget: W, shared_id: impl Into<ConstraintId<T>>, axis: Axis) -> Self {
    Self {
      child: WidgetPod::new(widget),
      shared_id: shared_id.into(),
      axis,
    }
  }

  fn layout_impl(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &T, env: &Env) -> Size {
    if let Some(constraint) = self.get_shared_data(data, env) {
      self.child.layout(
        ctx,
        &BoxConstraints::tight(Size::from(match self.axis {
          Axis::Horizontal => (constraint, bc.max().height),
          Axis::Vertical => (bc.max().width, constraint),
        })),
        data,
        env,
      )
    } else {
      let size = self.child.layout(ctx, bc, data, env);

      let unconstrained = match self.axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
      };
      if unconstrained.is_finite() {
        self.insert_potential_constraint(data, env, unconstrained)
      }

      size
    }
  }

  fn map_apply<U>(
    &self,
    data: &T,
    env: &Env,
    func: impl Fn(&mut SharedConstraintState) -> U,
  ) -> Option<U> {
    env.try_get(PARENT_LAYOUT_REPEATER_ID).ok().and_then(|id| {
      CONSTRAINT_MAP.with_borrow_mut(|map| {
        let state = map
          .entry((id, self.shared_id.resolve(data, env)))
          .or_default();
        Some(func(state))
      })
    })
  }

  fn get_shared_data(&self, data: &T, env: &Env) -> Option<f64> {
    self
      .map_apply(data, env, |state| state.constraint)
      .flatten()
  }

  fn insert_potential_constraint(&self, data: &T, env: &Env, value: f64) {
    self.map_apply(data, env, |state| state.values.push(value));
  }
}

#[cfg(test)]
mod test {
  use druid::widget::SizedBox;

  use super::{LayoutRepeater, SharedConstraintState, CONSTRAINT_MAP};

  #[test]
  fn clear_or_init_clears() {
    let parent: LayoutRepeater<(), SizedBox<_>> = LayoutRepeater::new(0, SizedBox::empty());

    CONSTRAINT_MAP.with_borrow_mut(|map| {
      let mut state = SharedConstraintState::default();
      state.values = vec![10.0; 6];

      let unaltered_state = state.clone();

      map.insert((0, 0), state.clone());
      map.insert((0, 1), state);
      map.insert((1, 0), unaltered_state);
    });

    parent.clear_or_init();

    CONSTRAINT_MAP.with_borrow(|map| {
      assert!(map.get(&(0, 0)).unwrap().values.is_empty());
      assert!(map.get(&(0, 1)).unwrap().values.is_empty());
      assert_eq!(map.get(&(1, 0)).unwrap().values, vec![10.0; 6]);
    })
  }
}

pub enum ConstraintId<T> {
  Fixed(u64),
  Dynamic(Box<dyn Fn(&T, &Env) -> u64>),
}

impl<T: Data, F: Fn(&T, &Env) -> u64 + 'static> From<F> for ConstraintId<T> {
  fn from(value: F) -> Self {
    Self::Dynamic(Box::new(value))
  }
}

impl<T: Data> From<u64> for ConstraintId<T> {
  fn from(value: u64) -> Self {
    Self::Fixed(value)
  }
}

impl<T> ConstraintId<T> {
  fn resolve(&self, data: &T, env: &Env) -> u64 {
    match self {
      ConstraintId::Fixed(fixed) => *fixed,
      ConstraintId::Dynamic(dynamic) => dynamic(data, env),
    }
  }
}

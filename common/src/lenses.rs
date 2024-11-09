use std::{borrow::Borrow, fmt::Debug, marker::PhantomData, rc::Rc};

use druid::{
  lens::{Identity, InArc, Map, Then},
  Data, Lens, LensExt as _,
};
use druid_widget_nursery::prism::{Closures, Prism};

pub trait LensExtExt<A: ?Sized, B: ?Sized>: Lens<A, B> + Sized {
  fn compute<Get, C>(self, get: Get) -> impl Lens<A, C>
  where
    Get: Fn(&B) -> C,
  {
    self.map(get, |_, _| {})
  }

  fn cloned(self) -> impl Lens<A, B>
  where
    B: Clone,
  {
    self.map(std::clone::Clone::clone, |b, a| b.clone_from(&a))
  }

  fn owned<C>(self) -> impl Lens<A, C>
  where
    B: ToOwned<Owned = C> + Clone,
    C: Borrow<B>,
  {
    self.map(std::borrow::ToOwned::to_owned, |b, c| {
      b.clone_from(c.borrow())
    })
  }

  fn debug<DBG>(self, dbg: DBG) -> Then<Self, Dbg<DBG>, B>
  where
    DBG: Fn(&B) + 'static,
    B: Clone,
  {
    self.then(Dbg(dbg))
  }

  fn convert<C>(self) -> Then<Self, Convert<B, C>, B>
  where
    B: From<C> + Clone,
    C: From<B> + Data,
  {
    self.then(Convert::<B, C>::new())
  }

  fn in_rc(self) -> InRc<Self>
  where
    A: Clone,
    B: Data,
  {
    InRc::new(self)
  }
}

impl<A: ?Sized, B: ?Sized, T: Lens<A, B>> LensExtExt<A, B> for T {}

#[derive(Clone)]
pub struct Compute<Get: Fn(&B) -> C, B: ?Sized, C>(Map<Get, fn(&mut B, C)>);

impl<Get: Fn(&B) -> C, B: ?Sized, C> Compute<Get, B, C> {
  pub fn new(f: Get) -> Self {
    Self(Map::new(f, |_, _| {}))
  }
}

impl<Get: Fn(&B) -> C, B: ?Sized, C> Lens<B, C> for Compute<Get, B, C> {
  fn with<V, F: FnOnce(&C) -> V>(&self, data: &B, f: F) -> V {
    self.0.with(data, f)
  }

  fn with_mut<V, F: FnOnce(&mut C) -> V>(&self, data: &mut B, f: F) -> V {
    self.0.with_mut(data, f)
  }
}

pub struct Dbg<DBG>(DBG);

impl<T, DBG: Fn(&T) + 'static> Lens<T, T> for Dbg<DBG> {
  fn with<V, F: FnOnce(&T) -> V>(&self, data: &T, f: F) -> V {
    self.0(data);
    f(data)
  }

  fn with_mut<V, F: FnOnce(&mut T) -> V>(&self, data: &mut T, f: F) -> V {
    self.0(data);
    f(data)
  }
}

#[derive(Clone)]
pub struct Convert<T, U> {
  outer: PhantomData<T>,
  inner: PhantomData<U>,
}

impl<T, U> Default for Convert<T, U> {
  fn default() -> Self {
    Self {
      outer: PhantomData,
      inner: PhantomData,
    }
  }
}

impl<T, U> Convert<T, U> {
  pub fn new() -> Self {
    Self::default()
  }
}

impl<T: From<U> + Clone, U: Data + From<T>> Lens<T, U> for Convert<T, U> {
  fn with<V, F: FnOnce(&U) -> V>(&self, data: &T, f: F) -> V {
    let data = data.clone().into();
    f(&data)
  }

  fn with_mut<V, F: FnOnce(&mut U) -> V>(&self, data: &mut T, f: F) -> V {
    let mut val = data.clone().into();
    let res = f(&mut val);
    *data = val.into();

    res
  }
}

/// A `Lens` that exposes data within an `Arc` with copy-on-write semantics
///
/// A copy is only made in the event that a different value is written.
#[derive(Debug, Copy, Clone)]
pub struct InRc<L> {
  inner: L,
}

impl<L> InRc<L> {
  /// Adapt a lens to operate on an `Arc`
  ///
  /// See also `LensExt::in_arc`
  pub fn new<A, B>(inner: L) -> Self
  where
    A: Clone,
    B: Data,
    L: Lens<A, B>,
  {
    Self { inner }
  }
}

impl<A, B, L> Lens<Rc<A>, B> for InRc<L>
where
  A: Clone,
  B: Data,
  L: Lens<A, B>,
{
  fn with<V, F: FnOnce(&B) -> V>(&self, data: &Rc<A>, f: F) -> V {
    self.inner.with(data, f)
  }

  fn with_mut<V, F: FnOnce(&mut B) -> V>(&self, data: &mut Rc<A>, f: F) -> V {
    let mut temp = self.inner.with(data, std::clone::Clone::clone);
    let v = f(&mut temp);
    if self.inner.with(data, |x| !x.same(&temp)) {
      self.inner.with_mut(Rc::make_mut(data), |x| *x = temp);
    }
    v
  }
}
pub fn ident_arc<T: Data>() -> InArc<Identity> {
  InArc::new::<T, T>(Identity)
}

#[must_use]
pub fn ident_rc<T: Data>() -> InRc<Identity> {
  InRc::new::<T, T>(Identity)
}

pub trait PrismExt<A, B>: Prism<A, B> {
  fn then_some<Other, C>(self, right: Other) -> ThenSome<Self, Other, B>
  where
    Other: Prism<B, C>,
    Self: Sized,
  {
    ThenSome::new(self, right)
  }
}

impl<A, B, T: Prism<A, B>> PrismExt<A, B> for T {}

#[derive(Clone)]
pub struct ThenSome<T, U, B> {
  left: T,
  right: U,
  _marker: PhantomData<B>,
}

impl<T, U, B> ThenSome<T, U, B> {
  pub fn new<A, C>(left: T, right: U) -> Self
  where
    T: Prism<A, B>,
    U: Prism<B, C>,
  {
    Self {
      left,
      right,
      _marker: PhantomData,
    }
  }
}

impl<T, U, A, B, C> Prism<A, C> for ThenSome<T, U, B>
where
  T: Prism<A, B>,
  U: Prism<B, C>,
{
  fn get(&self, data: &A) -> Option<C> {
    self.left.get(data).and_then(|b| self.right.get(&b))
  }

  fn put(&self, data: &mut A, inner: C) {
    let temp: Option<B> = self.left.get(data);
    if let Some(mut temp) = temp {
      self.right.put(&mut temp, inner);
      self.left.put(data, temp);
    }
  }
}

pub struct IsSome;

impl IsSome {
  pub fn new<B, F: Fn(&B) -> Option<B>>(func: F) -> Closures<F, fn(&mut B, B)> {
    Closures(func, |a, b| *a = b)
  }
}

pub struct PrismBox<T, U>(Box<dyn Prism<T, U>>);

impl<T, U> PrismBox<T, U> {
  pub fn new(prism: impl Prism<T, U> + 'static) -> Self {
    Self(Box::new(prism))
  }
}

impl<T, U> Prism<T, U> for PrismBox<T, U> {
  fn get(&self, data: &T) -> Option<U> {
    self.0.get(data)
  }

  fn put(&self, data: &mut T, inner: U) {
    self.0.put(data, inner);
  }
}

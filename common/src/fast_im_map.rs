use std::{borrow::Borrow, fmt::Debug, hash::Hash, ops::{Deref, DerefMut, Index, IndexMut}};

use druid::Data;

#[derive(Clone, Default)]
pub struct FastImMap<K, V>(druid::im::HashMap<K, V, ahash::RandomState>);

impl<K, V> FastImMap<K, V> {
  pub fn new() -> Self {
    Self(druid::im::HashMap::with_hasher(ahash::RandomState::new()))
  }

  pub fn inner(self) -> druid::im::HashMap<K, V, ahash::RandomState> {
    self.0
  }
}

impl<K: Clone + Hash + Eq, V: Clone> Debug for FastImMap<K, V>
where
  druid::im::HashMap<K, V, ahash::RandomState>: Debug,
{
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.deref().fmt(f)
  }
}

impl<K: Clone + Hash + Eq, V: Clone + Hash> Hash for FastImMap<K, V>
where
  druid::im::HashMap<K, V, ahash::RandomState>: Hash,
{
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.deref().hash(state);
  }
}

impl<K: Clone + Hash + Eq, V: Clone> Deref for FastImMap<K, V> {
  type Target = druid::im::HashMap<K, V, ahash::RandomState>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<K: Clone + Hash + Eq, V: Clone> DerefMut for FastImMap<K, V> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl<KB: Hash + Eq + ?Sized, K: Clone + Hash + Eq + Borrow<KB>, V: Clone> Index<&KB>
  for FastImMap<K, V>
{
  type Output = V;

  fn index(&self, index: &KB) -> &Self::Output {
    self.deref().index(index)
  }
}

impl<KB: Hash + Eq + ?Sized, K: Clone + Hash + Eq + Borrow<KB>, V: Clone> IndexMut<&KB>
  for FastImMap<K, V>
{
  fn index_mut(&mut self, index: &KB) -> &mut Self::Output {
    self.deref_mut().index_mut(index)
  }
}

impl<K: Clone + Eq + Hash + 'static, V: Clone + Data + 'static> Data for FastImMap<K, V> {
  fn same(&self, other: &Self) -> bool {
    self.is_submap_by(&**other, druid::Data::same)
  }
}

impl<K: Clone + Hash + Eq, V: Clone> From<FastImMap<K, V>>
  for druid::im::HashMap<K, V, ahash::RandomState>
{
  fn from(other: FastImMap<K, V>) -> Self {
    other.0
  }
}

impl<K: Clone + Hash + Eq + PartialEq + Eq, V: Clone, O: Into<druid::im::HashMap<K, V>>> From<O>
  for FastImMap<K, V>
{
  fn from(other: O) -> Self {
    let mut new = Self::new();
    new.extend(other.into().iter().map(|(k, v)| (k.clone(), v.clone())));

    new
  }
}

impl<K, V> FromIterator<(K, V)> for FastImMap<K, V>
where
  K: Hash + Eq + Clone,
  V: Clone,
{
  fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
    Self(iter.into_iter().collect())
  }
}

impl<K: Clone + Hash + Eq, V: Clone> PartialEq for FastImMap<K, V>
where
  druid::im::HashMap<K, V, ahash::RandomState>: PartialEq,
{
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl<K: Clone + Hash + Eq, V: Clone> Eq for FastImMap<K, V> where
  druid::im::HashMap<K, V, ahash::RandomState>: Eq
{
}

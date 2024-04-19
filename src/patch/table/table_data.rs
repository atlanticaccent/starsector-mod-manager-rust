use std::{
  fmt::Debug,
  hash::Hash,
  ops::{Deref, Index, IndexMut},
  rc::Rc,
  sync::Arc,
};

use druid::{
  im::{HashMap, Vector},
  lens::{Identity, InArc},
  Data, Widget, WidgetExt,
};

pub trait RowData: Data {
  type Id: Hash + Eq + Clone + Debug;
  type Column: Hash + Eq;

  fn id(&self) -> &Self::Id;

  fn cell(&self, column: &Self::Column) -> Box<dyn Widget<Self>>;
}

impl<T: RowData> RowData for Arc<T> {
  type Id = T::Id;
  type Column = T::Column;

  fn id(&self) -> &Self::Id {
    self.deref().id()
  }

  fn cell(&self, column: &Self::Column) -> Box<dyn Widget<Self>> {
    self
      .deref()
      .cell(column)
      .lens(InArc::new::<T, T>(Identity))
      .boxed()
  }
}

pub trait TableData:
  Data
  + for<'a> Index<&'a <Self::Row as RowData>::Id, Output = Self::Row>
  + for<'a> IndexMut<&'a <Self::Row as RowData>::Id, Output = Self::Row>
{
  type Row: RowData<Column = Self::Column>;
  type Column: Hash + Eq + Clone;

  fn keys(&self) -> impl Iterator<Item = <Self::Row as RowData>::Id>;

  fn columns(&self) -> impl Iterator<Item = Self::Column>;
}

impl<S: Data + Hash + Eq + Debug> RowData
  for (S, Vector<Rc<dyn Fn() -> Box<dyn Widget<()>> + 'static>>)
{
  type Id = S;
  type Column = usize;

  fn id(&self) -> &Self::Id {
    &self.0
  }

  fn cell(&self, column: &Self::Column) -> Box<dyn Widget<Self>> {
    (self.1[*column])().lens(druid::lens::Unit).boxed()
  }
}

impl<S: Data + Hash + Eq + Debug> TableData
  for HashMap<S, (S, Vector<Rc<dyn Fn() -> Box<dyn Widget<()>> + 'static>>)>
{
  type Row = (S, Vector<Rc<dyn Fn() -> Box<dyn Widget<()>> + 'static>>);
  type Column = usize;

  fn keys(&self) -> impl Iterator<Item = <Self::Row as RowData>::Id> {
    self.keys().cloned()
  }

  fn columns(&self) -> impl Iterator<Item = Self::Column> {
    (0..self.values().next().unwrap().1.len()).into_iter()
  }
}

#[extend::ext(name = ToTableData)]
pub impl<S: Data + Hash + Eq> HashMap<S, Vector<Rc<dyn Fn() -> Box<dyn Widget<()>> + 'static>>> {
  fn to_table_data(self) -> HashMap<S, (S, Vector<Rc<dyn Fn() -> Box<dyn Widget<()>> + 'static>>)> {
    self.into_iter().map(|(k, v)| (k.clone(), (k, v))).collect()
  }
}

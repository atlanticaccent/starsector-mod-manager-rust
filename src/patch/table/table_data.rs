use std::{
  fmt::Debug,
  hash::Hash,
  ops::{Deref, Index},
  sync::Arc,
};

use druid::{
  im::Vector,
  lens::{Identity, InArc},
  Data, Widget, WidgetExt,
};

pub trait RowData: Data {
  type Id: Hash + Eq + Clone + Debug;
  type Column: Hash + Eq + Debug;

  fn id(&self) -> Self::Id;

  fn cell(&self, column: &Self::Column) -> Box<dyn Widget<Self>>;
}

impl<T: RowData> RowData for Arc<T> {
  type Column = T::Column;
  type Id = T::Id;

  fn id(&self) -> Self::Id {
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

pub trait TableData: Data + Index<<Self::Row as RowData>::Id, Output = Self::Row> {
  type Row: RowData<Column = Self::Column>;
  type Column: Hash + Eq + Clone + Debug;

  fn keys(&self) -> impl Iterator<Item = <Self::Row as RowData>::Id>;

  fn columns(&self) -> impl Iterator<Item = Self::Column>;

  fn with_mut(&mut self, idx: <Self::Row as RowData>::Id, mutate: impl FnOnce(&mut Self::Row));
}

pub type WidgetFactoryRow = Vector<Arc<dyn Fn() -> Box<dyn Widget<()>>>>;

impl RowData for (usize, WidgetFactoryRow) {
  type Column = usize;
  type Id = usize;

  fn id(&self) -> Self::Id {
    self.0
  }

  fn cell(&self, column: &Self::Column) -> Box<dyn Widget<Self>> {
    (self.1[*column])().lens(druid::lens::Unit).boxed()
  }
}

pub type WidgetFactoryTable = Vector<(usize, WidgetFactoryRow)>;

impl TableData for WidgetFactoryTable {
  type Column = usize;
  type Row = (usize, WidgetFactoryRow);

  fn keys(&self) -> impl Iterator<Item = <Self::Row as RowData>::Id> {
    0..self.len()
  }

  fn columns(&self) -> impl Iterator<Item = Self::Column> {
    if self.is_empty() {
      0..0
    } else {
      0..self[0].1.len()
    }
  }

  fn with_mut(&mut self, idx: <Self::Row as RowData>::Id, mutate: impl FnOnce(&mut Self::Row)) {
    mutate(&mut self[idx])
  }
}

impl RowData for () {
  type Column = usize;
  type Id = usize;

  fn id(&self) -> Self::Id {
    0
  }

  fn cell(&self, _: &Self::Column) -> Box<dyn Widget<Self>> {
    unreachable!()
  }
}

impl TableData for [(); 0] {
  type Column = usize;
  type Row = ();

  fn keys(&self) -> impl Iterator<Item = <Self::Row as RowData>::Id> {
    0..0
  }

  fn columns(&self) -> impl Iterator<Item = Self::Column> {
    0..0
  }

  fn with_mut(&mut self, _: usize, _: impl FnOnce(&mut ())) {}
}

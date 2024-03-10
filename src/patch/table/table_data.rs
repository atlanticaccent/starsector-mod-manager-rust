use std::{
  fmt::Debug, hash::Hash, ops::{Deref, Index, IndexMut}, sync::Arc
};

use druid::{
  lens::{Identity, InArc},
  Data, Widget, WidgetExt,
};

pub trait RowData: Data {
  type Id: Hash + Eq + Clone + Debug;
  type Column: Hash + Eq + Debug;

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
    self.deref().cell(column)
      .lens(InArc::new::<T, T>(Identity))
      .boxed()
  }
}

pub trait TableData:
  Data
  + for<'a> Index<&'a <Self::Row as RowData>::Id, Output = Self::Row>
  + for<'a> IndexMut<&'a <Self::Row as RowData>::Id, Output = Self::Row>
{
  type Row: RowData<Column = Self::Column> + Debug;
  type Column: Hash + Eq + Clone + Debug;

  fn keys(&self) -> impl Iterator<Item = &<Self::Row as RowData>::Id>;

  fn columns(&self) -> impl Iterator<Item = &Self::Column>;
}

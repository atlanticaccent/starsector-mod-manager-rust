use std::ops::{Index, IndexMut};

use druid::{Data, Lens};

pub trait Column {
  type In;
  type Out;

  fn lens(&self) -> impl Lens<Self::In, Self::Out>;
}

pub trait RowData:
  Data
{
  type Id;

  fn id(&self) -> Self::Id;
}

pub trait TableData:
  Data
  + Index<<Self::Row as RowData>::Id, Output = Self::Row>
  + IndexMut<<Self::Row as RowData>::Id, Output = Self::Row>
{
  type Row: RowData;

  fn keys(&self) -> impl Iterator<Item = <Self::Row as RowData>::Id>;
}

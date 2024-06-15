use std::{
  cell::Cell,
  fmt::Display,
  hash::Hash,
  ops::{Deref, Index, IndexMut},
  rc::Rc,
};

use druid::{
  im::Vector, widget::WidgetWrapper, Data, Key, Lens, Selector, Widget, WidgetExt, WidgetId,
};

use super::ecs::EcsWidget;
use crate::{
  app::{
    controllers::LayoutRepeater,
    util::{xxHashMap, WidgetExtEx},
  },
  patch::table::{FlexTable, RowData, TableColumnWidth, TableData},
};

pub trait CellConstructor<T, U, W>: Fn(&T, U, fn(&druid::Env) -> usize) -> W {}

impl<T, U, W, F: Fn(&T, U, fn(&druid::Env) -> usize) -> W> CellConstructor<T, U, W> for F {}

pub trait WrapData: Data {
  type Id<'a>: ToOwned<Owned = Self::OwnedId>;
  type OwnedId: Eq + Clone + ToString;
  type Value;

  fn ids<'a>(&'a self) -> impl Iterator<Item = <Self::Id<'a> as ToOwned>::Owned>;

  fn len(&self) -> usize;
}

impl<K: Clone + Hash + Eq + Display + 'static, V: Data> WrapData for xxHashMap<K, V>
where
  for<'a> &'a K: ToOwned<Owned = K>,
{
  type Id<'a> = &'a K;
  type OwnedId = K;
  type Value = V;

  fn ids<'a>(&'a self) -> impl Iterator<Item = K> {
    self.keys().cloned()
  }

  fn len(&self) -> usize {
    self.deref().len()
  }
}

impl<T: Data> WrapData for Vector<T> {
  type Id<'a> = usize;
  type OwnedId = usize;
  type Value = T;

  fn ids<'a>(&'a self) -> impl Iterator<Item = usize> {
    0..self.len()
  }

  fn len(&self) -> usize {
    self.len()
  }
}

pub struct WrappedTable<T: WrapData, W: Widget<T> + 'static> {
  id: WidgetId,
  table: LayoutRepeater<TableDataImpl<T, W>, FlexTable<TableDataImpl<T, W>>>,
  min_width: f64,
  columns: usize,
  constructor: Rc<dyn CellConstructor<T, T::OwnedId, W>>,
  skip_paint: bool,
}

impl<T: WrapData, W: Widget<T> + 'static> WrappedTable<T, W> {
  const UPDATE_AND_LAYOUT: Selector<WidgetId> = Selector::new("wrapped_table.update_and_layout");

  pub fn new(min_width: f64, constructor: impl CellConstructor<T, T::OwnedId, W> + 'static) -> Self {
    Self {
      id: WidgetId::next(),
      table: FlexTable::new()
        .default_column_width((TableColumnWidth::Flex(1.0), min_width))
        .in_layout_repeater(),
      min_width,
      columns: 0,
      constructor: Rc::new(constructor),
      skip_paint: false,
    }
  }

  fn data_wrapper(&self, data: &T) -> TableDataImpl<T, W> {
    TableDataImpl {
      width: self.columns,
      data: RowDataImpl {
        data: data.clone(),
        data_ids: data.ids().collect(),
        width: self.columns,
        row: 0.into(),
        constructor: self.constructor.clone(),
      },
    }
  }
}

impl<T: WrapData, W: Widget<T> + 'static> Widget<T> for WrappedTable<T, W> {
  fn event(
    &mut self,
    ctx: &mut druid::EventCtx,
    event: &druid::Event,
    data: &mut T,
    env: &druid::Env,
  ) {
    let mut wrapped = self.data_wrapper(data);

    if let druid::Event::Command(cmd) = event
      && let Some(id) = cmd.get(Self::UPDATE_AND_LAYOUT)
      && *id == self.id
    {
      ctx.request_update();
    }

    self.table.event(ctx, event, &mut wrapped, env);
    *data = wrapped.data.data
  }

  fn lifecycle(
    &mut self,
    ctx: &mut druid::LifeCycleCtx,
    event: &druid::LifeCycle,
    data: &T,
    env: &druid::Env,
  ) {
    if let druid::LifeCycle::ViewContextChanged(_) = event {
      ctx.request_layout();
    }
    if let druid::LifeCycle::WidgetAdded = event {
      self.columns = data.len();

      ctx.request_layout()
    }

    self
      .table
      .lifecycle(ctx, event, &self.data_wrapper(data), env)
  }

  fn update(&mut self, ctx: &mut druid::UpdateCtx, old_data: &T, data: &T, env: &druid::Env) {
    let old_wrapper = self.data_wrapper(old_data);
    let wrapper = self.data_wrapper(data);
    self.table.update(ctx, &old_wrapper, &wrapper, env);
    if self.skip_paint {
      self.skip_paint = false;
      ctx.request_layout()
    }
  }

  fn layout(
    &mut self,
    ctx: &mut druid::LayoutCtx,
    bc: &druid::BoxConstraints,
    data: &T,
    env: &druid::Env,
  ) -> druid::Size {
    let columns = ((bc.max().width / self.min_width).floor() as usize)
      .min(data.len())
      .max(1);

    let mut wrapper = self.data_wrapper(data);
    let old_height = wrapper.height();
    let old_columns = self.columns;
    self.columns = columns;
    wrapper.width = columns;
    let height = wrapper.height();

    let table = WidgetWrapper::wrapped_mut(&mut self.table);
    let max_width = (bc.max().width - 10.0) / columns as f64;
    table.set_column_widths(&vec![max_width.into(); columns]);

    if height != old_height || self.columns != old_columns || self.skip_paint {
      ctx.submit_command(Self::UPDATE_AND_LAYOUT.with(self.id));
      self.skip_paint = true;
      bc.max()
    } else {
      self.table.layout(ctx, bc, &wrapper, env)
    }
  }

  fn paint(&mut self, ctx: &mut druid::PaintCtx, data: &T, env: &druid::Env) {
    if !self.skip_paint {
      self.table.paint(ctx, &self.data_wrapper(data), env)
    }
  }
}

#[derive(Lens)]
struct RowDataImpl<T: WrapData, W> {
  data: T,
  data_ids: Vec<T::OwnedId>,
  width: usize,
  row: Cell<usize>,
  constructor: Rc<dyn CellConstructor<T, T::OwnedId, W>>,
}

impl<T: WrapData, W> Clone for RowDataImpl<T, W> {
  fn clone(&self) -> Self {
    Self {
      data: self.data.clone(),
      data_ids: self.data_ids.clone(),
      width: self.width,
      row: self.row.clone(),
      constructor: self.constructor.clone(),
    }
  }
}

impl<T: WrapData, W: 'static> Data for RowDataImpl<T, W> {
  fn same(&self, other: &Self) -> bool {
    self.data.same(&other.data)
      && self.data_ids == other.data_ids
      && self.width.same(&other.width)
      && self.row.get().same(&other.row.get())
  }
}

impl<T: WrapData, W: Widget<T> + 'static> RowData for RowDataImpl<T, W> {
  type Id = usize;
  type Column = usize;

  fn id(&self) -> Self::Id {
    self.row.get()
  }

  fn cell(&self, column: &Self::Column) -> Box<dyn Widget<Self>> {
    let constructor = self.constructor.clone();
    let raw_id = get_id_raw(self.width as u64, self.id() as u64, *column as u64);
    let id = (raw_id < self.data.len()).then(|| self.data_ids[raw_id].to_owned());
    let data = self.data.clone();

    EcsWidget::new(
      move |data: &RowDataImpl<T, W>, env: &_| {
        let id = get_id(env);
        (id < data.data.len()).then(|| data.data_ids[id].to_string())
      },
      move || {
        constructor(&data, id.clone().unwrap(), get_id)
          .lens(RowDataImpl::data)
          .shared_constraint(
            |data: &RowDataImpl<T, W>, _: &druid::Env| data.row.get() as u64,
            druid::widget::Axis::Vertical,
          )
          .expand_width()
      },
    )
    .env_scope(|env, data| env.set(COL_NUM, data.width as u64))
    .boxed()
  }
}

struct TableDataImpl<T: WrapData, W> {
  width: usize,
  data: RowDataImpl<T, W>,
}

impl<T: WrapData, W> TableDataImpl<T, W> {
  fn height(&self) -> usize {
    let len = self.data.data.len();
    if len <= self.width {
      1
    } else {
      let mut height = len / self.width;
      if len % self.width > 0 {
        height += 1
      }

      height
    }
  }
}

impl<T: WrapData, W> Index<usize> for TableDataImpl<T, W> {
  type Output = RowDataImpl<T, W>;

  fn index(&self, row: usize) -> &Self::Output {
    self.data.row.set(row);
    &self.data
  }
}

impl<T: WrapData, W> IndexMut<usize> for TableDataImpl<T, W> {
  fn index_mut(&mut self, row: usize) -> &mut Self::Output {
    self.data.row.set(row);
    &mut self.data
  }
}

impl<T: WrapData, W> Clone for TableDataImpl<T, W> {
  fn clone(&self) -> Self {
    Self {
      width: self.width.clone(),
      data: self.data.clone(),
    }
  }
}

impl<T: WrapData, W: 'static> Data for TableDataImpl<T, W> {
  fn same(&self, other: &Self) -> bool {
    self.data.same(&other.data)
      && self.height().same(&other.height())
      && self.width.same(&other.width)
  }
}

impl<T: WrapData, W: Widget<T> + 'static> TableData for TableDataImpl<T, W> {
  type Row = RowDataImpl<T, W>;
  type Column = usize;

  fn keys(&self) -> impl Iterator<Item = <Self::Row as crate::patch::table::RowData>::Id> {
    0..self.height()
  }

  fn columns(&self) -> impl Iterator<Item = Self::Column> {
    0..self.width
  }
}

const COL_NUM: Key<u64> = Key::new("wrapped_table.cell.col_num");

fn get_id(env: &druid::Env) -> usize {
  get_id_inner(env.get(COL_NUM), env)
}

fn get_id_inner(width: u64, env: &druid::Env) -> usize {
  ((env.get(FlexTable::<[(); 0]>::ROW_IDX) * width) + env.get(FlexTable::<[(); 0]>::COL_IDX))
    as usize
}

fn get_id_raw(width: u64, row: u64, col: u64) -> usize {
  (width * row + col) as usize
}

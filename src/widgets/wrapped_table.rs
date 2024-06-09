use std::{
  ops::{Index, IndexMut},
  rc::Rc,
};

use druid::{
  lens, widget::WidgetWrapper, Data, Key, Lens, Selector, Widget, WidgetExt, WidgetId,
};

use super::ecs::EcsWidget;
use crate::{
  app::{
    controllers::{HeightLinkerShared, LayoutRepeater, LinkedHeights},
    util::WidgetExtEx,
  },
  patch::table::{FlexTable, RowData, TableColumnWidth, TableData},
};

pub trait CellConstructor<W>: Fn(usize, fn(&druid::Env) -> usize) -> W {}

impl<W, F: Fn(usize, fn(&druid::Env) -> usize) -> W> CellConstructor<W> for F {}

pub trait IndexableData: Data + Index<usize> {}

impl<T: Data + Index<usize>> IndexableData for T {}

pub struct WrappedTable<T: IndexableData, W: Widget<T> + 'static>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
  id: WidgetId,
  table: LayoutRepeater<TableDataImpl<T, W>, FlexTable<TableDataImpl<T, W>>>,
  min_width: f64,
  columns: usize,
  constructor: Rc<dyn CellConstructor<W>>,
  skip_paint: bool,
}

impl<T: IndexableData, W: Widget<T> + 'static> WrappedTable<T, W>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
  const UPDATE_AND_LAYOUT: Selector<WidgetId> = Selector::new("wrapped_table.update_and_layout");

  pub fn new(min_width: f64, constructor: impl CellConstructor<W> + 'static) -> Self {
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
        width: self.columns,
        constructor: self.constructor.clone(),
      },
    }
  }
}

impl<T: IndexableData, W: Widget<T> + 'static> Widget<T> for WrappedTable<T, W>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
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
      self.columns = data.into_iter().len();

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
      .min(data.into_iter().len())
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
struct RowDataImpl<T, W> {
  data: T,
  width: usize,
  constructor: Rc<dyn CellConstructor<W>>,
}

impl<T: Clone, W> Clone for RowDataImpl<T, W> {
  fn clone(&self) -> Self {
    Self {
      data: self.data.clone(),
      width: self.width,
      constructor: self.constructor.clone(),
    }
  }
}

impl<T: IndexableData, W: 'static> Data for RowDataImpl<T, W> {
  fn same(&self, other: &Self) -> bool {
    self.data.same(&other.data) && self.width.same(&other.width)
  }
}

impl<T: IndexableData, W: Widget<T> + 'static> RowData for RowDataImpl<T, W>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
  type Id = usize;
  type Column = usize;

  fn id(&self) -> Self::Id {
    0
  }

  fn cell(&self, column: &Self::Column) -> Box<dyn Widget<Self>> {
    let column = *column;
    let constructor = self.constructor.clone();

    EcsWidget::new(
      |(col_num, data): &(usize, T), env: &druid::Env| {
        let id = get_id_inner(*col_num as u64, env);
        (data.into_iter().len() > id).then_some(id)
      },
      move || {
        constructor(column, get_id)
          .shared_constraint(
            |_: &_, env: &druid::Env| env.get(FlexTable::<[(); 0]>::ROW_IDX),
            druid::widget::Axis::Vertical,
          )
          .expand_width()
          .lens(lens!((usize, T), 1))
          .env_scope(|env, (col_num, _)| env.set(COL_NUM, *col_num as u64))
          .border(druid::Color::TRANSPARENT, 1.0)
      },
    )
    .lens(druid::lens::Map::new(
      |row_data: &Self| (row_data.width, row_data.data.clone()),
      |receiver, (_, data)| receiver.data = data,
    ))
    .boxed()
  }
}

struct TableDataImpl<T, W> {
  width: usize,
  data: RowDataImpl<T, W>,
}

impl<T, W> TableDataImpl<T, W>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
  fn height(&self) -> usize {
    let len = self.data.data.into_iter().len();
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

impl<T, W> Index<usize> for TableDataImpl<T, W>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
  type Output = RowDataImpl<T, W>;

  fn index(&self, _: usize) -> &Self::Output {
    &self.data
  }
}

impl<T, W> IndexMut<usize> for TableDataImpl<T, W>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
  fn index_mut(&mut self, _: usize) -> &mut Self::Output {
    &mut self.data
  }
}

impl<T: Clone, W> Clone for TableDataImpl<T, W> {
  fn clone(&self) -> Self {
    Self {
      width: self.width.clone(),
      data: self.data.clone(),
    }
  }
}

impl<T: IndexableData, W: 'static> Data for TableDataImpl<T, W>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
  fn same(&self, other: &Self) -> bool {
    self.data.same(&other.data)
      && self.height().same(&other.height())
      && self.width.same(&other.width)
  }
}

impl<T: IndexableData, W: Widget<T> + 'static> TableData for TableDataImpl<T, W>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
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

const HEIGHT_LINKER_SYNC: Selector<(usize, HeightLinkerShared)> =
  Selector::new("wrapped_table.height_linker.sync");

fn on_linked_height_sync<T: Data, W: Widget<T>>(
  height_linker: &mut LinkedHeights<T, W>,
  ctx: &mut druid::EventCtx,
  (target_row, linker): &(usize, HeightLinkerShared),
  _data: &mut T,
  env: &druid::Env,
) -> bool {
  let row = env.get(FlexTable::<[(); 0]>::ROW_IDX) as usize;

  if *target_row == row {
    height_linker.reset_local_state();
    height_linker.set_height_linker_inner(linker.clone());
    ctx.request_layout();
  }

  true
}

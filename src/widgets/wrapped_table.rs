use std::{
  ops::{Index, IndexMut},
  rc::Rc,
};

use druid::{Data, Lens, Selector, Widget, WidgetExt, WidgetId, WidgetPod};

use super::ecs::EcsWidget;
use crate::patch::table::{FlexTable, RowData, TableColumnWidth, TableData};

pub trait IndexableData: Data + Index<usize> {}

impl<T: Data + Index<usize>> IndexableData for T {}

pub struct WrappedTable<T: IndexableData, W: Widget<T> + 'static>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
  table: WidgetPod<TableDataImpl<T, W>, FlexTable<TableDataImpl<T, W>>>,
  min_width: f64,
  columns: usize,
  constructor: Rc<dyn Fn(fn(&druid::Env) -> usize) -> W>,
  skip_paint: bool,
}

impl<T: IndexableData, W: Widget<T> + 'static> WrappedTable<T, W>
where
  for<'a> &'a T: IntoIterator,
  for<'a> <&'a T as IntoIterator>::IntoIter: ExactSizeIterator,
{
  const UPDATE_AND_LAYOUT: Selector<WidgetId> = Selector::new("wrapped_table.update_and_layout");

  pub fn new(
    min_width: f64,
    constructor: impl Fn(fn(&druid::Env) -> usize) -> W + 'static,
  ) -> Self {
    Self {
      table: WidgetPod::new(
        FlexTable::new().default_column_width((TableColumnWidth::Flex(1.0), min_width)),
      ),
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
    if let druid::Event::Command(cmd) = event
      && let Some(id) = cmd.get(Self::UPDATE_AND_LAYOUT)
      && *id == self.table.id()
    {
      ctx.request_update();
      // ctx.request_layout();
    }

    let mut wrapped = self.data_wrapper(data);
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
    if let druid::LifeCycle::WidgetAdded = event {
      self.columns = data.into_iter().len()
    }

    self
      .table
      .lifecycle(ctx, event, &self.data_wrapper(data), env)
  }

  fn update(&mut self, ctx: &mut druid::UpdateCtx, _old_data: &T, data: &T, env: &druid::Env) {
    let wrapper = self.data_wrapper(data);
    self.table.update(ctx, &wrapper, env);
    self.skip_paint = false;
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
    self.columns = columns;
    wrapper.width = columns;

    if wrapper.height() > old_height || self.skip_paint {
      ctx.submit_command(Self::UPDATE_AND_LAYOUT.with(self.table.id()));
      self.skip_paint = true;
      self.table.layout_rect().size()
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
  constructor: Rc<dyn Fn(fn(&druid::Env) -> usize) -> W>,
}

impl<T: Clone, W> Clone for RowDataImpl<T, W> {
  fn clone(&self) -> Self {
    Self {
      data: self.data.clone(),
      constructor: self.constructor.clone(),
    }
  }
}

impl<T: IndexableData, W: 'static> Data for RowDataImpl<T, W> {
  fn same(&self, other: &Self) -> bool {
    self.data.same(&other.data)
  }
}

impl<T: IndexableData, W: Widget<T> + 'static> RowData for RowDataImpl<T, W> {
  type Id = usize;
  type Column = usize;

  fn id(&self) -> Self::Id {
    0
  }

  fn cell(&self, _column: &Self::Column) -> Box<dyn Widget<Self>> {
    let constructor = self.constructor.clone();
    EcsWidget::new(
      |_: &_, env: &druid::Env| {
        (env.get(FlexTable::<[(); 0]>::ROW_IDX) + env.get(FlexTable::<[(); 0]>::COL_IDX)) as usize
      },
      move || constructor(get_id),
    )
    .lens(RowDataImpl::data)
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

fn get_id(env: &druid::Env) -> usize {
  (env.get(FlexTable::<[(); 0]>::ROW_IDX) + env.get(FlexTable::<[(); 0]>::COL_IDX)) as usize
}

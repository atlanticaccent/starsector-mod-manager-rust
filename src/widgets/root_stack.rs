use std::rc::Rc;

use druid::{
  lens,
  widget::{SizedBox, ViewSwitcher},
  Data, EventCtx, Lens, Point, Selector, SingleUse, Widget, WidgetExt as _,
};
use druid_widget_nursery::{
  CommandCtx, Stack, StackChildParams, StackChildPosition, WidgetExt as _,
};

use crate::app::{util::WidgetExtEx as _, App};

#[derive(Clone, Data, Lens)]
pub struct RootStack {
  pub(crate) widget_maker: Option<Rc<Box<dyn Fn() -> Box<dyn Widget<crate::app::App>>>>>,
  pub(crate) on_dismiss: Option<Rc<Box<dyn Fn(&mut EventCtx)>>>,
  pub(crate) position: StackChildPosition,
}

impl RootStack {
  pub(crate) const SHOW: Selector<
    SingleUse<(
      Point,
      Box<dyn Fn() -> Box<dyn Widget<App>>>,
      Option<Box<dyn Fn(&mut EventCtx)>>,
    )>,
  > = Selector::new("root_stack.new");
  pub(crate) const DISMISS: Selector = Selector::new("root_stack.dismiss");

  pub fn new(widget: impl Widget<App> + 'static) -> impl Widget<App> {
    Stack::new()
      .with_child(
        widget
          .lens(lens!((App, RootStack), 0))
          .on_click(|ctx, data, _| {
            data.1.widget_maker = None;
            if let Some(on_dismiss) = data.1.on_dismiss.take() {
              on_dismiss(ctx)
            }
          }),
      )
      .with_positioned_child(
        ViewSwitcher::new(
          |data: &(App, RootStack), _| data.1.widget_maker.clone(),
          |maker, _, _| {
            if let Some(maker) = maker {
              maker().lens(lens!((App, RootStack), 0)).boxed()
            } else {
              SizedBox::empty().boxed()
            }
          },
        ),
        StackChildParams::dynamic(|data: &(App, RootStack), _| &data.1.position).duration(0.0),
      )
      .on_command(Self::SHOW, |ctx, payload, data| {
        if !data.0.block_next_root_stack {
          let payload = payload.take().unwrap();
          data.1.position = StackChildPosition::new()
            .left(Some(payload.0.x))
            .top(Some(payload.0.y));
          data.1.widget_maker = Some(Rc::new(payload.1));
          data.1.on_dismiss = payload.2.map(Rc::new);
          ctx.request_update();
        } else {
          data.0.block_next_root_stack = false;
        }
      })
      .on_command(Self::DISMISS, |ctx, _, data| {
        data.1.widget_maker = None;
        if let Some(on_dismiss) = data.1.on_dismiss.take() {
          on_dismiss(ctx)
        }
      })
      .scope(
        |app| {
          (
            app,
            RootStack {
              widget_maker: None,
              position: StackChildPosition::new(),
              on_dismiss: None,
            },
          )
        },
        lens!((App, RootStack), 0),
      )
  }

  pub fn show(
    ctx: &mut impl CommandCtx,
    point: Point,
    widget_maker: impl Fn() -> Box<dyn Widget<App>> + 'static,
    on_dismiss: Option<impl Fn(&mut EventCtx) + 'static>,
  ) {
    ctx.submit_command(Self::SHOW.with(SingleUse::new((
      point,
      Box::new(widget_maker),
      on_dismiss.map(|fun| Box::new(fun) as Box<dyn Fn(&mut EventCtx)>),
    ))))
  }

  pub fn dismiss(ctx: &mut impl CommandCtx) {
    ctx.submit_command(Self::DISMISS)
  }
}

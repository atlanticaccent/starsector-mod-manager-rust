use std::rc::Rc;

use druid::{
  lens,
  widget::{SizedBox, ViewSwitcher},
  Data, EventCtx, Lens, Point, Selector, SingleUse, Widget, WidgetExt as _,
};
use druid_widget_nursery::{
  CommandCtx, Stack, StackChildParams, StackChildPosition, WidgetExt as _,
};

use crate::widget_ext::WidgetExtEx as _;

type WidgetMaker<A> = Rc<Box<dyn Fn() -> Box<dyn Widget<A>>>>;

type DismissCallback = Rc<Box<dyn Fn(&mut EventCtx)>>;

#[derive(Clone, Data, Lens)]
pub struct RootStack<A> {
  pub(crate) widget_maker: Option<WidgetMaker<A>>,
  pub(crate) on_dismiss: Option<DismissCallback>,
  pub(crate) position: StackChildPosition,
}

type RootStackConstructor<A> = SingleUse<(
  Point,
  Box<dyn Fn() -> Box<dyn Widget<A>>>,
  Option<Box<dyn Fn(&mut EventCtx)>>,
)>;

impl<A: Data> RootStack<A> {
  pub const SHOW: Selector<RootStackConstructor<A>> = Selector::new("root_stack.new");
  pub const DISMISS: Selector = Selector::new("root_stack.dismiss");

  pub fn new(widget: impl Widget<A> + 'static) -> impl Widget<A> {
    Stack::new()
      .with_child(
        widget
          .lens(lens!((A, RootStack<A>), 0))
          .on_click(|ctx, data, _| {
            data.1.widget_maker = None;
            if let Some(on_dismiss) = data.1.on_dismiss.take() {
              on_dismiss(ctx);
            }
          }),
      )
      .with_positioned_child(
        ViewSwitcher::new(
          |data: &(A, RootStack<A>), _| data.1.widget_maker.clone(),
          |maker, _, _| {
            if let Some(maker) = maker {
              maker().lens(lens!((A, RootStack<A>), 0)).boxed()
            } else {
              SizedBox::empty().boxed()
            }
          },
        ),
        StackChildParams::dynamic(|data: &(A, RootStack<A>), _| &data.1.position).duration(0.0),
      )
      .on_command(Self::SHOW, |ctx, payload, data| {
        let payload = payload.take().unwrap();
        data.1.position = StackChildPosition::new()
          .left(Some(payload.0.x))
          .top(Some(payload.0.y));
        data.1.widget_maker = Some(Rc::new(payload.1));
        data.1.on_dismiss = payload.2.map(Rc::new);
        ctx.request_update();
      })
      .on_command(Self::DISMISS, |ctx, (), data| {
        data.1.widget_maker = None;
        if let Some(on_dismiss) = data.1.on_dismiss.take() {
          on_dismiss(ctx);
        }
      })
      .lens_scope(
        |app| {
          (app, RootStack {
            widget_maker: None,
            position: StackChildPosition::new(),
            on_dismiss: None,
          })
        },
        lens!((A, RootStack<A>), 0),
      )
  }

  pub fn show(
    ctx: &mut impl CommandCtx,
    point: Point,
    widget_maker: impl Fn() -> Box<dyn Widget<A>> + 'static,
    on_dismiss: Option<impl Fn(&mut EventCtx) + 'static>,
  ) {
    ctx.submit_command(Self::SHOW.with(SingleUse::new((
      point,
      Box::new(widget_maker),
      on_dismiss.map(|fun| Box::new(fun) as Box<dyn Fn(&mut EventCtx)>),
    ))));
  }

  pub fn dismiss(ctx: &mut impl CommandCtx) {
    ctx.submit_command(Self::DISMISS);
  }
}

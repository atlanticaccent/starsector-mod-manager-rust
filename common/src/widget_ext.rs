use std::marker::PhantomData;

use druid::{
  keyboard_types,
  lens::{Constant, Identity, Unit},
  widget::{
    Align, Axis, ControllerHost, DefaultScopePolicy, Either, LensScopeTransfer, LensWrap, Scope,
    ScopeTransfer, SizedBox,
  },
  Command, Data, Env, Event, EventCtx, Lens, LensExt as _, MouseEvent, Selector, Target, UnitPoint,
  Widget, WidgetExt, WidgetId,
};
use druid_patch::click::Click;
use druid_widget_nursery::{
  prism::{Prism, PrismWrap},
  stack_tooltip::StackTooltip,
  CommandCtx, Mask,
};

use crate::{
  controllers::{
    next_id, BoxedOnEvent, CommandFn, ConstraintId, DelayedPainter, ExtensibleController,
    HeightLinkerShared, HoverState, InvisibleIf, LayoutRepeater, LinkedHeights, OnCmd, OnEvent,
    OnHover, OnNotif, SharedConstraint, SharedIdHoverState,
  },
  lens,
  widgets::card::{Card, CardBuilder},
};

pub trait WidgetExtEx<T: Data, W: Widget<T>>: Widget<T> + Sized + 'static {
  fn on_notification<CT: 'static, F: Fn(&mut EventCtx, &CT, &mut T) + 'static>(
    self,
    selector: Selector<CT>,
    handler: F,
  ) -> ControllerHost<Self, OnNotif<CT, T, F>> {
    self.controller(OnNotif::new(selector, handler))
  }

  fn on_click2(
    self,
    f: impl Fn(&mut EventCtx, &MouseEvent, &mut T, &Env) + 'static,
  ) -> ControllerHost<Self, Click<T>> {
    ControllerHost::new(self, Click::new(f))
  }

  /**
   * Sets the event as handled if the callback returns true
   */
  fn on_event<F: Fn(&mut W, &mut EventCtx, &Event, &mut T) -> bool + 'static>(
    self,
    f: F,
  ) -> ControllerHost<Self, OnEvent<T, W, F>> {
    ControllerHost::new(self, OnEvent::new(f))
  }

  /**
   * Displays alternative when closure returns false
   */
  fn empty_if_not(self, f: impl Fn(&T, &Env) -> bool + 'static) -> Either<T> {
    Either::new(f, self, SizedBox::empty())
  }

  /**
   * Displays alternative when closure returns true
   */
  fn empty_if(self, f: impl Fn(&T, &Env) -> bool + 'static) -> Either<T> {
    Either::new(f, SizedBox::empty(), self)
  }

  fn else_if(
    self,
    f: impl Fn(&T, &Env) -> bool + 'static,
    other: impl Widget<T> + 'static,
  ) -> Either<T> {
    Either::new(f, other, self)
  }

  /// Execute closure when command is received, with mutable access to the child
  /// widget.
  /// * Must return bool indicating if the event should be propgated to the
  ///   child - true to propagate, false to not.
  fn on_command2<CT: 'static>(
    self,
    selector: Selector<CT>,
    handler: impl Fn(&mut W, &mut EventCtx, &CT, &mut T) -> bool + 'static,
  ) -> ControllerHost<Self, OnCmd<CT, T, W>> {
    ControllerHost::new(
      self,
      OnCmd::new(selector, CommandFn::Plain(Box::new(handler))),
    )
  }

  /// Execute closure when command is received, with mutable access to the child
  /// widget.
  /// * Must return bool indicating if the event should be propgated to the
  ///   child - true to propagate, false to not.
  fn on_command3<CT: 'static>(
    self,
    selector: Selector<CT>,
    handler: impl Fn(&mut W, &mut EventCtx, &CT, &mut T, &Env) -> bool + 'static,
  ) -> ControllerHost<Self, OnCmd<CT, T, W>> {
    ControllerHost::new(
      self,
      OnCmd::new(selector, CommandFn::WithEnv(Box::new(handler))),
    )
  }

  fn link_height_with(
    self,
    height_linker: &mut Option<HeightLinkerShared>,
  ) -> LinkedHeights<T, Self> {
    if let Some(linker) = height_linker {
      LinkedHeights::new(self, linker)
    } else {
      let (widget, linker) = LinkedHeights::new_with_linker(self);
      height_linker.replace(linker);

      widget
    }
  }

  fn link_height_unwrapped(self, height_linker: &HeightLinkerShared) -> LinkedHeights<T, Self> {
    LinkedHeights::new(self, height_linker)
  }

  fn on_hover(
    self,
    handler: impl Fn(&mut W, &mut EventCtx, &mut T) -> bool + 'static,
  ) -> ControllerHost<Self, BoxedOnEvent<T, W>> {
    ControllerHost::new(self, OnHover::new(handler))
  }

  fn with_z_index(self, z_index: u32) -> DelayedPainter<T, Self> {
    DelayedPainter::new(self, z_index)
  }

  fn valign_centre(self) -> Align<T> {
    self.align_vertical(UnitPoint::CENTER)
  }

  fn halign_centre(self) -> Align<T> {
    self.align_horizontal(UnitPoint::CENTER)
  }

  fn prism<U, P: Prism<U, T>>(self, prism: P) -> PrismWrap<Self, P, T> {
    PrismWrap::new(self, prism)
  }

  fn constant<U: Data>(self, constant: T) -> LensWrap<U, T, Constant<T>, Self> {
    self.lens(Constant(constant))
  }

  fn scope<U: Data, In: FnOnce(U) -> T>(
    self,
    make_state: In,
    read: impl Fn(&mut T, &U) + 'static,
    write: impl Fn(&T, &mut U) + 'static,
  ) -> impl Widget<U> {
    Scope::from_function(make_state, FnTransfer::new(read, write), self)
  }

  fn partial_scope<
    P: Data,
    U: Data,
    F: FnOnce(U) -> T + 'static,
    LS: Lens<T, P> + Clone + 'static,
    LI: Lens<U, P> + Clone + 'static,
  >(
    self,
    make_state: F,
    lens_state: LS,
    lens_in: LI,
  ) -> impl Widget<U> {
    Scope::from_function(
      Box::new(make_state),
      PartialScopeTransfer::new(lens_state, lens_in),
      self,
    )
  }

  fn lens_scope<U: Data, In: Fn(U) -> T, L: Lens<T, U>>(
    self,
    make_state: In,
    lens: L,
  ) -> Scope<DefaultScopePolicy<In, LensScopeTransfer<L, U, T>>, Self> {
    Scope::from_lens(make_state, lens, self)
  }

  fn scope_independent<U: Data, In: Fn() -> T + 'static>(self, make_state: In) -> impl Widget<U> {
    Scope::from_lens(
      Box::new(move |()| make_state()) as Box<dyn Fn(()) -> T>,
      Unit,
      self,
    )
    .lens(Identity.then(Unit))
  }

  fn scope_indie_computed<U: Data, In: Fn(U) -> T + 'static>(
    self,
    make_state: In,
  ) -> impl Widget<U> {
    Scope::from_function(make_state, DummyTransfer::default(), self)
  }

  fn invisible_if(self, func: impl Fn(&T, &Env) -> bool + 'static) -> InvisibleIf<T, Self> {
    InvisibleIf::new(func, self)
  }

  fn invisible(self) -> InvisibleIf<T, Self> {
    InvisibleIf::new(|_, _| true, self)
  }

  fn on_key_up(
    self,
    key: keyboard_types::Key,
    func: impl Fn(&mut EventCtx, &mut T) -> bool + 'static,
  ) -> ControllerHost<Self, BoxedOnEvent<T, W>> {
    self.on_event(Box::new(move |_, ctx, event, data| {
      if let Event::KeyUp(key_event) = event
        && key_event.key == key
      {
        func(ctx, data)
      } else {
        false
      }
    }))
  }

  fn suppress_event(
    self,
    matches: impl Fn(&Event) -> bool + 'static,
  ) -> ControllerHost<Self, BoxedOnEvent<T, W>> {
    self.on_event(Box::new(move |_, _, event, _| matches(event)))
  }

  fn disabled(self) -> impl Widget<T> {
    self.on_added(|_, ctx, _, _| ctx.set_disabled(true))
  }

  fn in_card(self) -> impl Widget<T> {
    Card::new(self)
  }

  fn in_card_builder(self, builder: CardBuilder) -> impl Widget<T> {
    builder.build(self)
  }

  fn scope_with<U: Data, In: Fn(T) -> U, SWO: Widget<State<T, U>> + 'static>(
    self,
    state_maker: In,
    with: impl FnOnce(LensWrap<State<T, U>, T, state_derived_lenses::outer<T, U>, Self>) -> SWO,
  ) -> impl Widget<T> {
    let inner = self.lens(<State<T, U>>::outer);
    Scope::from_lens(
      move |outer| State {
        outer: outer.clone(),
        inner: state_maker(outer),
      },
      <State<T, U>>::outer,
      with(inner),
    )
  }

  fn mask_default(self) -> Mask<T> {
    Mask::new(self).show_mask(true)
  }

  fn shared_constraint(
    self,
    id: impl Into<ConstraintId<T>>,
    axis: Axis,
  ) -> SharedConstraint<T, Self> {
    SharedConstraint::new(self, id, axis)
  }

  fn in_layout_repeater(self) -> LayoutRepeater<T, Self> {
    LayoutRepeater::new(next_id(), self)
  }

  fn stack_tooltip_custom(self, label: impl Widget<T> + 'static) -> StackTooltip<T> {
    StackTooltip::custom(self, label)
      .with_background_color(druid::Color::TRANSPARENT)
      .with_border_color(druid::Color::TRANSPARENT)
      .with_border_width(0.0)
  }

  fn wrap_with_hover_state<S: HoverState>(self, state: S, set_cursor: bool) -> impl Widget<T> {
    self.scope_with_hover_state(state, set_cursor, |widget| widget)
  }

  fn scope_with_hover_state<S: HoverState, WO: Widget<(T, S)> + 'static>(
    self,
    state: S,
    set_cursor: bool,
    scope: impl FnOnce(Box<dyn Widget<(T, S)>>) -> WO,
  ) -> impl Widget<T> {
    scope(self.lens(lens!((T, S), 0)).boxed()).with_hover_state_opts(state, set_cursor)
  }

  fn on_lifecycle(
    self,
    func: impl Fn(&mut Self, &mut druid::LifeCycleCtx, &druid::LifeCycle, &T, &Env) + 'static,
  ) -> ControllerHost<Self, ExtensibleController<T, Self>> {
    self.controller(ExtensibleController::new().on_lifecycle(func))
  }
}

#[derive(Clone, Data, Lens)]
pub struct State<Outer, Inner> {
  pub outer: Outer,
  pub inner: Inner,
}

impl<T: Data, W: Widget<T> + 'static> WidgetExtEx<T, W> for W {}

pub const HOVER_STATE_CHANGE: Selector = Selector::new("util.hover_state.change");

pub trait WithHoverState<S: HoverState + Data + Clone, T: Data, W: Widget<(T, S)> + 'static>:
  Widget<(T, S)> + Sized + 'static
{
  fn with_hover_state(self, state: S) -> Box<dyn Widget<T>> {
    self.with_hover_state_opts(state, true)
  }

  fn with_hover_state_opts(self, state: S, set_cursor: bool) -> Box<dyn Widget<T>> {
    let id = WidgetId::next();

    Scope::from_lens(
      move |data| (data, state.clone()),
      lens!((T, S), 0),
      self
        .on_event(move |_, ctx, event, data| {
          if let druid::Event::MouseMove(_) = event
            && !ctx.is_disabled()
          {
            if set_cursor {
              ctx.override_cursor(&druid::Cursor::Pointer);
            }
            data.1.set(true);
            ctx.request_update();
            ctx.request_paint();
          } else if let druid::Event::Command(cmd) = event
            && cmd.is(HOVER_STATE_CHANGE)
          {
            data.1.set(false);
            if set_cursor {
              ctx.clear_cursor();
            }
          }
          ctx.request_update();
          ctx.request_paint();
          false
        })
        .with_id(id)
        .controller(
          ExtensibleController::new().on_lifecycle(move |_, ctx, event, _, _| {
            if let druid::LifeCycle::HotChanged(false) = event {
              ctx.submit_command(HOVER_STATE_CHANGE.to(id));
            }
          }),
        ),
    )
    .boxed()
  }
}

impl<S: HoverState + Data + Clone, T: Data, W: Widget<(T, S)> + 'static> WithHoverState<S, T, W>
  for W
{
}

pub trait WithHoverIdState<T: Data, W: Widget<(T, SharedIdHoverState)> + 'static>:
  Widget<(T, SharedIdHoverState)> + Sized + 'static
{
  fn with_shared_id_hover_state(self, state: SharedIdHoverState) -> Box<dyn Widget<T>> {
    self.with_shared_id_hover_state_opts(state, false)
  }

  fn with_shared_id_hover_state_opts(
    self,
    state: SharedIdHoverState,
    set_cursor: bool,
  ) -> Box<dyn Widget<T>> {
    const HOVER_STATE_CHANGE_FOR_ID: Selector<(WidgetId, bool)> =
      Selector::new("util.hover_state.change_for_id");

    let id = state.0;
    Scope::from_lens(
      move |data| (data, state.clone()),
      lens!((T, SharedIdHoverState), 0),
      self
        .on_event(move |_, ctx, event, data| {
          if let druid::Event::MouseMove(_) = event
            && !ctx.is_disabled()
          {
            if set_cursor {
              ctx.set_cursor(&druid::Cursor::Pointer);
            }
            data.1.set(true);
            ctx.request_update();
            ctx.request_paint();
          } else if let druid::Event::Command(cmd) = event
            && let Some((target, state)) = cmd.get(HOVER_STATE_CHANGE_FOR_ID)
            && *target == id
          {
            data.1.set(*state);
            if set_cursor {
              if *state {
                ctx.set_cursor(&druid::Cursor::Pointer);
              } else {
                ctx.clear_cursor();
              }
            }
            ctx.request_update();
            ctx.request_paint();
          }
          false
        })
        .controller(
          ExtensibleController::new().on_lifecycle(move |_, ctx, event, _, _| {
            if let druid::LifeCycle::HotChanged(state) = event {
              ctx.submit_command(HOVER_STATE_CHANGE_FOR_ID.with((id, *state)));
            }
          }),
        ),
    )
    .boxed()
  }
}

impl<T: Data, W: Widget<(T, SharedIdHoverState)> + 'static> WithHoverIdState<T, W> for W {}

pub trait TransferRead<State, In> = Fn(&mut State, &In);
pub trait TransferWrite<State, In> = Fn(&State, &mut In);

pub struct FnTransfer<
  In: Data,
  State: Data,
  R: TransferRead<State, In>,
  W: TransferWrite<State, In>,
> {
  read: R,
  write: W,
  _read: PhantomData<In>,
  _write: PhantomData<State>,
}

impl<In: Data, State: Data, R: TransferRead<State, In>, W: TransferWrite<State, In>>
  FnTransfer<In, State, R, W>
{
  pub fn new(read: R, write: W) -> Self {
    Self {
      read,
      write,
      _read: PhantomData,
      _write: PhantomData,
    }
  }
}

impl<In: Data, State: Data, R: TransferRead<State, In>, W: TransferWrite<State, In>> ScopeTransfer
  for FnTransfer<In, State, R, W>
{
  type In = In;
  type State = State;

  fn read_input(&self, state: &mut Self::State, input: &Self::In) {
    (self.read)(state, input);
  }

  fn write_back_input(&self, state: &Self::State, input: &mut Self::In) {
    (self.write)(state, input);
  }
}

// TODO: macro that syncs fields with same names between two structs using
// existing lens impls on tuples of lenses

pub struct PartialScopeTransfer<In, State> {
  read: Box<dyn TransferRead<State, In>>,
  write: Box<dyn TransferWrite<State, In>>,
}

impl<In, State> PartialScopeTransfer<In, State> {
  pub fn new<Prt: Data>(
    lens_state: impl Lens<State, Prt> + Clone + 'static,
    lens_in: impl Lens<In, Prt> + Clone + 'static,
  ) -> PartialScopeTransfer<In, State> {
    PartialScopeTransfer {
      read: {
        let lens_state = lens_state.clone();
        let lens_in = lens_in.clone();
        Box::new(move |state: &mut State, data: &In| {
          let partial = lens_in.with(data, std::clone::Clone::clone);
          lens_state.with_mut(state, |inner| {
            if !inner.same(&partial) {
              *inner = partial;
            }
          });
        })
      },
      write: Box::new(move |state, data| {
        let partial = lens_state.with(state, std::clone::Clone::clone);
        lens_in.with_mut(data, |inner| {
          if !inner.same(&partial) {
            *inner = partial;
          }
        });
      }),
    }
  }
}

impl<In: Data, State: Data> ScopeTransfer for PartialScopeTransfer<In, State> {
  type In = In;
  type State = State;

  fn read_input(&self, state: &mut State, data: &In) {
    (self.read)(state, data);
  }

  fn write_back_input(&self, state: &State, data: &mut In) {
    (self.write)(state, data);
  }
}
pub struct DummyTransfer<X, Y> {
  phantom_x: PhantomData<X>,
  phantom_y: PhantomData<Y>,
}

impl<X: Data, Y: Data> ScopeTransfer for DummyTransfer<X, Y> {
  type In = X;
  type State = Y;

  fn read_input(&self, _: &mut Self::State, _: &Self::In) {}

  fn write_back_input(&self, _: &Self::State, _: &mut Self::In) {}
}

impl<X, Y> Default for DummyTransfer<X, Y> {
  fn default() -> Self {
    Self {
      phantom_x: PhantomData,
      phantom_y: PhantomData,
    }
  }
}

pub trait CommandExt: CommandCtx {
  fn submit_command_global(&mut self, cmd: impl Into<Command>) {
    let cmd: Command = cmd.into();
    self.submit_command(cmd.to(Target::Global));
  }
}

impl<T: CommandCtx> CommandExt for T {}

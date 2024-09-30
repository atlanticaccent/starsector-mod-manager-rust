use std::{cell::RefCell, rc::Rc};

use druid::{
  kurbo::Circle,
  lens,
  piet::ScaleMode,
  theme,
  widget::{BackgroundBrush, Either, Painter},
  Color, Data, Env, Insets, KeyOrValue, LinearGradient, RadialGradient, Rect, RenderContext,
  UnitPoint, Widget, WidgetExt, WidgetId,
};

use super::card_button::ScopedStackCardButton;
use crate::{
  app::util::{ShadeColor, State, WithHoverState as _},
  theme::SHADOW,
};

pub struct Card;

impl Card {
  pub const CARD_INSET: f64 = 12.5;
  pub const DEFAULT_INSETS: (f64, f64) = (0.0, 14.0);

  #[must_use]
  pub fn builder() -> CardBuilder {
    CardBuilder::new()
  }

  pub fn hoverable<T: Data, W: Widget<T> + 'static, F>(
    widget_maker: F,
    insets: impl Into<Insets>,
  ) -> Box<dyn Widget<T>>
  where
    F: Fn() -> W,
  {
    Self::hoverable_distinct(
      &widget_maker,
      &widget_maker,
      &CardBuilder::new().with_insets(insets),
    )
  }

  pub fn hoverable_distinct<T: Data, W1: Widget<T> + 'static, W2: Widget<T> + 'static, F, FH>(
    unhovered: F,
    hovered: FH,
    builder: &CardBuilder,
  ) -> Box<dyn Widget<T>>
  where
    F: Fn() -> W1,
    FH: Fn() -> W2,
  {
    let card = |shadow| builder.clone().with_shadow_length(shadow);

    Either::new(
      |data: &(T, bool), _| data.1,
      card(builder.shadow_length.unwrap_or(6.0) + builder.shadow_increase.unwrap_or(2.0))
        .build(hovered())
        .lens(lens!((T, bool), 0)),
      card(builder.shadow_length.unwrap_or(6.0))
        .build(unhovered())
        .lens(lens!((T, bool), 0))
        .on_added(|_, ctx, _, _| {
          ctx.request_layout();
          ctx.request_paint();
        }),
    )
    .with_hover_state_opts(false, builder.set_cursor)
  }

  pub fn new<T: Data>(widget: impl Widget<T> + 'static) -> impl Widget<T> {
    Self::new_with_opts(
      widget,
      Self::DEFAULT_INSETS,
      4.0,
      6.0,
      None,
      Option::<(f64, Color)>::None,
      None,
    )
  }

  pub fn new_with_opts<T: Data>(
    widget: impl Widget<T> + 'static,
    padding: impl Into<Insets> + Clone,
    corner_radius: f64,
    shadow_length: f64,
    background: Option<BackgroundBrush<T>>,
    border: Option<(f64, impl Into<KeyOrValue<Color>>)>,
    on_hover: Option<BackgroundBrush<T>>,
  ) -> impl Widget<T> {
    let mut insets: Insets = padding.into();
    let mut paint_insets = insets;

    if paint_insets.x0 <= 0.0 {
      paint_insets.x0 = insets.y0.max(insets.y1);
      insets.x0 = paint_insets.x0 / 2.0;
    }
    if paint_insets.x1 <= 0.0 {
      paint_insets.x1 = insets.y0.max(insets.y1);
      insets.x1 = paint_insets.x1 / 2.0;
    }

    widget.padding(insets).background(Self::card_painter(
      paint_insets,
      corner_radius,
      shadow_length,
      background,
      border,
      on_hover,
    ))
  }

  pub fn card_painter<T: Data>(
    insets: impl Into<KeyOrValue<Insets>>,
    corner_radius: f64,
    shadow_length: f64,
    mut background: Option<BackgroundBrush<T>>,
    border: Option<(f64, impl Into<KeyOrValue<Color>>)>,
    mut on_hover: Option<BackgroundBrush<T>>,
  ) -> Painter<T> {
    let insets = insets.into();
    let border = border.map(|b| (b.0, b.1.into()));
    Painter::new(move |ctx, data, env| {
      let mut insets: Insets = insets.resolve(env);
      let border = border.as_ref().map(|b| (b.0, b.1.resolve(env)));

      let size = ctx.size();
      insets.x0 /= 2.0;
      insets.x1 /= 2.0;
      insets.y0 /= 2.0;
      insets.y1 /= 2.0;

      Self::shadow_painter(ctx, env, insets, corner_radius, shadow_length);

      let rounded_rect = size.to_rect().inset(-insets).to_rounded_rect(corner_radius);

      if let Some(background) = ctx
        .is_hot()
        .then_some(())
        .and(on_hover.as_mut())
        .or(background.as_mut())
      {
        ctx.with_save(|ctx| {
          ctx.clip(rounded_rect);
          background.paint(ctx, data, env);
        });
      } else {
        ctx.fill(rounded_rect, &env.get(theme::BACKGROUND_LIGHT));
      }

      if let Some((width, key)) = border {
        let shape = size
          .to_rect()
          .inset(-insets)
          .inset(-(width / 2.0))
          .to_rounded_rect(corner_radius - width);
        ctx.stroke(shape, &key, width);
      }
    })
  }

  fn shadow_painter(
    ctx: &mut druid::PaintCtx,
    env: &Env,
    insets: Insets,
    corner_radius: f64,
    shadow_length: f64,
  ) {
    let rect = ctx.size().to_rect();

    let light = Color::TRANSPARENT;
    let dark = env.try_get(SHADOW).unwrap_or(Color::BLACK);

    ctx.fill(rect, &light);

    let stops = (dark, dark.interpolate_with(light, 9), light);
    let radius = corner_radius + shadow_length;
    let mut circle = Circle::new(
      (insets.x0 + corner_radius, insets.y0 + corner_radius),
      radius,
    );
    let brush = RadialGradient::new(0.5, stops).with_scale_mode(ScaleMode::Fill);

    ctx.with_save(|ctx| {
      ctx.clip(Rect::new(0.0, 0.0, circle.center.x, circle.center.y));
      ctx.fill(circle, &brush);
    });
    circle.center.x += rect.width() - insets.x1 - insets.x0 - (corner_radius * 2.0);
    ctx.with_save(|ctx| {
      ctx.clip(Rect::new(
        circle.center.x,
        0.0,
        rect.width(),
        circle.center.y,
      ));
      ctx.fill(circle, &brush);
    });
    circle.center.y += rect.height() - insets.y1 - insets.y0 - (corner_radius * 2.0);
    ctx.with_save(|ctx| {
      ctx.clip(Rect::new(
        circle.center.x,
        circle.center.y,
        rect.width(),
        rect.height(),
      ));
      ctx.fill(circle, &brush);
    });
    circle.center.x -= rect.width() - insets.x0 - insets.x1 - (corner_radius * 2.0);
    ctx.with_save(|ctx| {
      ctx.clip(Rect::new(
        0.0,
        circle.center.y,
        circle.center.x,
        rect.height(),
      ));
      ctx.fill(circle, &brush);
    });

    let stops = (dark, light);
    let linear = LinearGradient::new(UnitPoint::BOTTOM, UnitPoint::TOP, stops);
    let rect = Rect::new(
      insets.x0 + corner_radius,
      insets.y0 + corner_radius,
      ctx.size().width - insets.x1 - corner_radius,
      insets.y0 + corner_radius - radius,
    );
    ctx.fill(rect, &linear);

    let linear = LinearGradient::new(UnitPoint::LEFT, UnitPoint::RIGHT, stops);
    let rect = Rect::new(
      ctx.size().width - insets.x1 - corner_radius,
      insets.y0 + corner_radius,
      (ctx.size().width - insets.x1 - corner_radius) + radius,
      ctx.size().height - insets.y1 - corner_radius,
    );
    ctx.fill(rect, &linear);

    let linear = LinearGradient::new(UnitPoint::TOP, UnitPoint::BOTTOM, stops);
    let rect = Rect::new(
      insets.x0 + corner_radius,
      ctx.size().height - insets.y1 - corner_radius,
      ctx.size().width - insets.x1 - corner_radius,
      (ctx.size().height - insets.y1 - corner_radius) + radius,
    );
    ctx.fill(rect, &linear);

    let linear = LinearGradient::new(UnitPoint::RIGHT, UnitPoint::LEFT, stops);
    let rect = Rect::new(
      insets.x0 + corner_radius,
      insets.y0 + corner_radius,
      insets.x0 + corner_radius - radius,
      ctx.size().height - insets.y1 - corner_radius,
    );
    ctx.fill(rect, &linear);
  }
}

pub struct CardBuilder {
  insets: Insets,
  corner_radius: f64,
  shadow_length: Option<f64>,
  border: Option<(f64, KeyOrValue<Color>)>,
  background: Option<BrushOrPainter>,
  on_hover: Option<BrushOrPainter>,
  shadow_increase: Option<f64>,
  set_cursor: bool,
}

enum BrushOrPainter {
  Brush(BackgroundBrush<()>),
  Painter(Rc<RefCell<Painter<()>>>),
}

impl Clone for BrushOrPainter {
  fn clone(&self) -> Self {
    match self {
      Self::Brush(brush) => Self::Brush(CardBuilder::partial_brush_clone(brush)),
      Self::Painter(cell) => Self::Painter(cell.clone()),
    }
  }
}

impl Clone for CardBuilder {
  fn clone(&self) -> Self {
    CardBuilder {
      insets: self.insets,
      corner_radius: self.corner_radius,
      shadow_length: self.shadow_length,
      border: self.border.clone(),
      background: self.background.clone(),
      on_hover: self.on_hover.clone(),
      shadow_increase: self.shadow_increase,
      set_cursor: self.set_cursor,
    }
  }
}

impl CardBuilder {
  fn new() -> Self {
    Self {
      insets: Card::DEFAULT_INSETS.into(),
      corner_radius: 4.0,
      shadow_length: None,
      border: None,
      background: None,
      on_hover: None,
      shadow_increase: None,
      set_cursor: true,
    }
  }

  pub fn with_insets(mut self, insets: impl Into<Insets>) -> Self {
    self.insets = insets.into();

    self
  }

  #[must_use]
  pub fn with_corner_radius(mut self, corner_radius: f64) -> Self {
    self.corner_radius = corner_radius;

    self
  }

  #[must_use]
  pub fn with_shadow_length(mut self, shadow_length: f64) -> Self {
    self.shadow_length = Some(shadow_length);

    self
  }

  #[must_use]
  pub fn with_shadow_increase(mut self, shadow_length_increase: f64) -> Self {
    self.shadow_increase = Some(shadow_length_increase);

    self
  }

  pub fn with_border(mut self, width: f64, color: impl Into<KeyOrValue<Color>>) -> Self {
    self.border = Some((width, color.into()));

    self
  }

  pub fn with_hover_background(mut self, background: impl Into<BackgroundBrush<()>>) -> Self {
    let background = match background.into() {
      BackgroundBrush::Painter(painter) => BrushOrPainter::Painter(Rc::new(RefCell::new(painter))),
      brush => BrushOrPainter::Brush(brush),
    };

    self.on_hover = Some(background);

    self
  }

  pub fn with_background(mut self, background: impl Into<BackgroundBrush<()>>) -> Self {
    let background = match background.into() {
      BackgroundBrush::Painter(painter) => BrushOrPainter::Painter(Rc::new(RefCell::new(painter))),
      brush => BrushOrPainter::Brush(brush),
    };

    self.background = Some(background);

    self
  }

  #[must_use]
  pub fn with_set_cursor(mut self, set_cursor: bool) -> Self {
    self.set_cursor = set_cursor;

    self
  }

  pub fn build<T: Data>(self, widget: impl Widget<T> + 'static) -> impl Widget<T> {
    Card::new_with_opts(
      widget,
      self.insets,
      self.corner_radius,
      self.shadow_length.unwrap_or(8.0),
      Self::convert(self.background),
      self.border,
      Self::convert(self.on_hover),
    )
  }

  pub fn hoverable<T: Data, W: Widget<T> + 'static>(
    self,
    widget_builder: impl Fn(bool) -> W,
  ) -> impl Widget<T> {
    self.hoverable_distinct(|| widget_builder(false), || widget_builder(true))
  }

  pub fn hoverable_distinct<T: Data, W1: Widget<T> + 'static, W2: Widget<T> + 'static, F, FH>(
    self,
    unhovered: F,
    hovered: FH,
  ) -> Box<dyn Widget<T>>
  where
    F: Fn() -> W1,
    FH: Fn() -> W2,
  {
    Card::hoverable_distinct(unhovered, hovered, &self)
  }

  pub fn stacked_button<
    T: Data,
    W: Widget<T> + 'static,
    WO: Widget<crate::app::App> + 'static,
    WS: Widget<State<T, bool>> + 'static,
  >(
    self,
    base_builder: impl Fn(bool) -> W + 'static,
    stack_builder: impl Fn(bool) -> WO + 'static,
    alt_stack_activation: Option<
      impl Fn(
          ScopedStackCardButton<T>,
          Rc<dyn Fn() -> Box<dyn Widget<crate::app::App>> + 'static>,
          WidgetId,
        ) -> WS
        + 'static,
    >,
    width: f64,
  ) -> impl Widget<T> {
    use super::card_button::CardButton;

    CardButton::stacked_dropdown_with_options(
      base_builder,
      stack_builder,
      alt_stack_activation,
      width,
      self,
    )
  }

  fn partial_brush_clone<T: Data, U: Data>(brush: &BackgroundBrush<T>) -> BackgroundBrush<U> {
    match brush {
      BackgroundBrush::Color(inner) => BackgroundBrush::Color(*inner),
      BackgroundBrush::ColorKey(inner) => BackgroundBrush::ColorKey(inner.clone()),
      BackgroundBrush::Linear(inner) => BackgroundBrush::Linear(inner.clone()),
      BackgroundBrush::Radial(inner) => BackgroundBrush::Radial(inner.clone()),
      BackgroundBrush::Fixed(inner) => BackgroundBrush::Fixed(inner.clone()),
      BackgroundBrush::Painter(_) => unreachable!(),
      _ => unimplemented!(),
    }
  }

  fn convert<U: Data>(brush: Option<BrushOrPainter>) -> Option<BackgroundBrush<U>> {
    brush.map(|brush| match brush {
      BrushOrPainter::Painter(cell) => {
        BackgroundBrush::Painter(Painter::new(move |ctx, _, env| {
          cell.borrow_mut().paint(ctx, &(), env);
        }))
      }
      BrushOrPainter::Brush(brush) => Self::partial_brush_clone(&brush),
    })
  }
}

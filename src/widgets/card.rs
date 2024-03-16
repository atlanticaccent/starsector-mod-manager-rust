use druid::{
  kurbo::Circle,
  lens,
  piet::ScaleMode,
  theme,
  widget::{BackgroundBrush, Either, Painter},
  Color, Data, Insets, KeyOrValue, LinearGradient, RadialGradient, Rect, RenderContext, UnitPoint,
  Widget, WidgetExt,
};

use crate::app::util::WithHoverState as _;

pub struct Card;

impl Card {
  pub const CARD_INSET: f64 = 12.5;
  pub const DEFAULT_INSETS: (f64, f64) = (0.0, 14.0);

  pub fn builder<T: Data>() -> CardBuilder<T> {
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
      || widget_maker(),
      || widget_maker(),
      CardBuilder::new().with_insets(insets),
    )
  }

  pub fn hoverable_distinct<T: Data, W1: Widget<T> + 'static, W2: Widget<T> + 'static, F, FH>(
    unhovered: F,
    hovered: FH,
    builder: CardBuilder<T>,
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
        .lens(lens!((T, bool), 0)),
    )
    .with_hover_state(false)
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
    let mut paint_insets = insets.clone();

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

      Self::shadow_painter(ctx, insets, corner_radius, shadow_length);

      let rounded_rect = size.to_rect().inset(-insets).to_rounded_rect(corner_radius);

      if let Some(background) = ctx
        .is_hot()
        .then_some(())
        .and_then(|_| on_hover.as_mut())
        .or_else(|| background.as_mut())
      {
        ctx.with_save(|ctx| {
          ctx.clip(rounded_rect);
          background.paint(ctx, data, env)
        });
      } else {
        ctx.fill(rounded_rect, &env.get(theme::BACKGROUND_LIGHT))
      }

      if let Some((width, key)) = border {
        let shape = size
          .to_rect()
          .inset(-insets)
          .inset(-(width / 2.0))
          .to_rounded_rect(corner_radius - width);
        ctx.stroke(shape, &key, width)
      }
    })
  }

  fn shadow_painter(
    ctx: &mut druid::PaintCtx,
    insets: Insets,
    corner_radius: f64,
    shadow_length: f64,
  ) {
    let rect = ctx.size().to_rect();

    let light = Color::TRANSPARENT;
    let dark = Color::BLACK;

    ctx.fill(rect, &light);

    let stops = (dark, light);
    let radius = corner_radius + shadow_length;
    let mut circle = Circle::new(
      (insets.x0 + corner_radius, insets.y0 + corner_radius),
      radius,
    );
    let brush = RadialGradient::new(0.5, stops).with_scale_mode(ScaleMode::Fit);

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

pub struct CardBuilder<T: Data> {
  insets: Insets,
  corner_radius: f64,
  shadow_length: Option<f64>,
  border: Option<(f64, KeyOrValue<Color>)>,
  background: Option<BackgroundBrush<T>>,
  on_hover: Option<BackgroundBrush<T>>,
  shadow_increase: Option<f64>,
}

impl<T: Data> Clone for CardBuilder<T> {
  fn clone(&self) -> Self {
    let clone_brush = |brush: Option<&BackgroundBrush<T>>| match brush {
      Some(brush) => Some(match brush {
        BackgroundBrush::Color(inner) => BackgroundBrush::Color(inner.clone()),
        BackgroundBrush::ColorKey(inner) => BackgroundBrush::ColorKey(inner.clone()),
        BackgroundBrush::Linear(inner) => BackgroundBrush::Linear(inner.clone()),
        BackgroundBrush::Radial(inner) => BackgroundBrush::Radial(inner.clone()),
        BackgroundBrush::Fixed(inner) => BackgroundBrush::Fixed(inner.clone()),
        BackgroundBrush::Painter(_) => unimplemented!(),
        _ => todo!(),
      }),
      None => None,
    };

    Self {
      insets: self.insets.clone(),
      corner_radius: self.corner_radius.clone(),
      shadow_length: self.shadow_length.clone(),
      border: self.border.clone(),
      background: clone_brush(self.background.as_ref()),
      on_hover: clone_brush(self.on_hover.as_ref()),
      shadow_increase: self.shadow_increase.clone(),
    }
  }
}

impl<T: Data> CardBuilder<T> {
  fn new() -> Self {
    Self {
      insets: Card::DEFAULT_INSETS.into(),
      corner_radius: 4.0,
      shadow_length: None,
      border: None,
      background: None,
      on_hover: None,
      shadow_increase: None,
    }
  }

  pub fn with_insets(mut self, insets: impl Into<Insets>) -> Self {
    self.insets = insets.into();

    self
  }

  pub fn with_corner_radius(mut self, corner_radius: f64) -> Self {
    self.corner_radius = corner_radius;

    self
  }

  pub fn with_shadow_length(mut self, shadow_length: f64) -> Self {
    self.shadow_length = Some(shadow_length);

    self
  }

  pub fn with_shadow_increase(mut self, shadow_length_increase: f64) -> Self {
    self.shadow_increase = Some(shadow_length_increase);

    self
  }

  pub fn with_border(mut self, width: f64, color: impl Into<KeyOrValue<Color>>) -> Self {
    self.border = Some((width, color.into()));

    self
  }

  pub fn with_hover_background(mut self, background: impl Into<BackgroundBrush<T>>) -> Self {
    self.on_hover = Some(background.into());

    self
  }

  pub fn with_background(mut self, background: impl Into<BackgroundBrush<T>>) -> Self {
    self.background = Some(background.into());

    self
  }

  pub fn build(self, widget: impl Widget<T> + 'static) -> impl Widget<T> {
    Card::new_with_opts(
      widget,
      self.insets,
      self.corner_radius,
      self.shadow_length.unwrap_or(8.0),
      self.background,
      self.border,
      self.on_hover,
    )
  }

  pub fn hoverable<W: Widget<T> + 'static>(self, widget_builder: impl Fn() -> W) -> impl Widget<T> {
    self.hoverable_distinct(|| widget_builder(), || widget_builder())
  }

  pub fn hoverable_distinct<W1: Widget<T> + 'static, W2: Widget<T> + 'static, F, FH>(
    self,
    unhovered: F,
    hovered: FH,
  ) -> Box<dyn Widget<T>>
  where
    F: Fn() -> W1,
    FH: Fn() -> W2,
  {
    Card::hoverable_distinct(unhovered, hovered, self)
  }
}

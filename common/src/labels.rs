use std::{convert::identity, sync::Arc};

use druid::{
  lens::{Constant, Map},
  text::{ArcStr, Attribute, RichText},
  theme,
  widget::{ControllerHost, Label, LabelText, LensWrap, Painter, RawLabel},
  Color, Data, Env, FontWeight, KeyOrValue, Lens, Selector, Widget, WidgetExt,
};

use crate::{
  controllers::HoverController,
  lenses::Compute,
  theme_keys::{BLUE_KEY, ON_BLUE_KEY},
  widget_ext::WidgetExtEx as _,
};

pub trait LabelExt<T: Data> {
  fn wrapped(label: impl AsRef<str>) -> Label<T> {
    Label::new(label.as_ref()).with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }

  fn wrapped_lens<U: Data, L: Lens<T, U>>(lens: L) -> LensWrap<T, String, L, Label<String>> {
    LensWrap::new(
      Label::dynamic(|t: &String, _| t.to_string())
        .with_line_break_mode(druid::widget::LineBreaking::WordWrap),
      lens,
    )
  }

  fn wrapped_func<F, S>(func: F) -> Label<T>
  where
    S: Into<Arc<str>>,
    F: Fn(&T, &druid::Env) -> S + 'static,
  {
    Label::new(func).with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }

  fn wrapped_into(label: impl Into<LabelText<T>>) -> Label<T> {
    Label::new(label).with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }

  fn stringify() -> Label<T>
  where
    T: ToString,
  {
    Label::new(|t: &T, _: &Env| t.to_string())
  }

  fn stringify_wrapped() -> Label<T>
  where
    T: ToString,
  {
    Label::stringify().with_line_break_mode(druid::widget::LineBreaking::WordWrap)
  }
}

impl<T: Data> LabelExt<T> for Label<T> {}

fn to_rich_text(
  text: impl AsRef<str>,
  size: impl Into<KeyOrValue<f64>>,
  weight: FontWeight,
  colour: impl Into<KeyOrValue<Color>>,
) -> RichText {
  RichText::new(text.as_ref().into())
    .with_attribute(0.., Attribute::Weight(weight))
    .with_attribute(0.., Attribute::FontSize(size.into()))
    .with_attribute(0.., Attribute::TextColor(colour.into()))
}

pub fn bold_text<T: Data>(
  text: &str,
  size: impl Into<KeyOrValue<f64>>,
  weight: FontWeight,
  colour: impl Into<KeyOrValue<Color>>,
) -> impl Widget<T> {
  RawLabel::new()
    .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
    .lens(Constant(to_rich_text(text, size, weight, colour)))
}

pub fn h1_fixed<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(text, 24., FontWeight::BOLD, theme::TEXT_COLOR)
}

pub fn h2_fixed<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(text, 20., FontWeight::SEMI_BOLD, theme::TEXT_COLOR)
}

pub fn h3_fixed<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(text, 18., FontWeight::MEDIUM, theme::TEXT_COLOR)
}

pub fn bolded<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(
    text,
    theme::TEXT_SIZE_NORMAL,
    FontWeight::MEDIUM,
    theme::TEXT_COLOR,
  )
}

pub fn lensed_bold<T: Data + AsRef<str>>(
  size: impl Into<KeyOrValue<f64>>,
  weight: FontWeight,
  colour: impl Into<KeyOrValue<Color>>,
) -> impl Widget<T> {
  let size = size.into();
  let colour = colour.into();
  RawLabel::new()
    .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
    .lens(Map::new(
      move |text| to_rich_text(text, size.clone(), weight, colour.clone()),
      |_, _| {},
    ))
}

pub fn h1<T: Data + AsRef<str>>() -> impl Widget<T> {
  lensed_bold(24., FontWeight::BOLD, theme::TEXT_COLOR)
}

pub fn h2<T: Data + AsRef<str>>() -> impl Widget<T> {
  lensed_bold(20., FontWeight::SEMI_BOLD, theme::TEXT_COLOR)
}

pub fn h3<T: Data + AsRef<str>>() -> impl Widget<T> {
  lensed_bold(18., FontWeight::MEDIUM, theme::TEXT_COLOR)
}

pub fn hoverable_text(
  colour: Option<impl Into<KeyOrValue<Color>> + 'static>,
) -> impl Widget<String> {
  hoverable_text_opts(colour, identity, &[], &[], false)
}

pub fn hoverable_text_opts<TXT: Into<ArcStr> + Data, W: Widget<RichText> + 'static>(
  colour: Option<impl Into<KeyOrValue<Color>> + 'static>,
  mut modify: impl FnMut(RawLabel<RichText>) -> W,
  attrs: &'static [Attribute],
  hover_attrs: &'static [Attribute],
  set_cursor: bool,
) -> impl Widget<TXT> {
  let colour = colour
    .map(Into::into)
    .unwrap_or_else(|| theme::TEXT_COLOR.into());

  let label: RawLabel<RichText> = RawLabel::new()
    .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
    .with_text_color(colour);

  let wrapped = modify(label);

  wrapped
    .lens(Compute::new(|(txt, _): &(Option<RichText>, _)| {
      txt.clone().unwrap()
    }))
    .scope_with_hover_state(false, set_cursor, move |widget| {
      widget.lens(Compute::new(
        move |((plain, hovered), is_hover): &((Option<RichText>, _), bool)| {
          let text = if *is_hover { hovered } else { plain };

          ((text.clone(), None), *is_hover)
        },
      ))
    })
    .lens(Compute::new(move |text: &TXT| {
      let rich = RichText::new(text.clone().into());
      let plain = attrs.iter().fold(
        rich.with_attribute(.., Attribute::Underline(true)),
        |txt, attr| txt.with_attribute(.., attr.clone()),
      );
      let hovered = hover_attrs.iter().fold(plain.clone(), |txt, attr| {
        txt.with_attribute(.., attr.clone())
      });

      (Some(plain), Some(hovered))
    }))
}

pub fn hyperlink_fn<TXT: Into<ArcStr> + Data + Clone>(
  selector: Selector<String>,
) -> impl Fn() -> ControllerHost<druid::widget::Container<TXT>, HoverController> + 'static {
  move || hyperlink_opts(selector)
}

pub fn hyperlink_opts<TXT: Into<ArcStr> + Data + Clone>(
  selector: Selector<String>,
) -> ControllerHost<druid::widget::Container<TXT>, HoverController> {
  hoverable_text_opts(
    Some(BLUE_KEY),
    identity,
    &[druid::text::Attribute::Weight(
      druid::text::FontWeight::SEMI_BOLD,
    )],
    &[druid::text::Attribute::TextColor(druid::KeyOrValue::Key(
      ON_BLUE_KEY,
    ))],
    true,
  )
  .on_click(move |ctx, data: &mut TXT, _| {
    let data: ArcStr = data.clone().into();
    ctx.submit_command(selector.with(data.to_string()))
  })
  .background(Painter::new(|ctx, _, env| {
    use druid::RenderContext;

    let size = ctx.size();
    if ctx.is_hot() {
      ctx.fill(size.to_rect(), &env.get(BLUE_KEY));
    }
  }))
  .controller(HoverController::new(false, true))
}

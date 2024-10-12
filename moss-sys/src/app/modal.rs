use std::cell::Cell;

use druid::{
  commands, theme,
  widget::{Button, Flex, Label, Painter},
  Command, Data, Env, LinearGradient, RenderContext, Selector, Target, Widget, WidgetExt,
  WindowConfig, WindowId,
};
use druid_widget_nursery::{AnyCtx, RequestCtx, Stack, StackChildPosition};
use indexmap::IndexMap;

use super::util::{h3_fixed, DragWindowController, LabelExt, WidgetExtEx};
use crate::app::util::Tap as _;

pub trait OnCloseCallback<T> {
  const NOOP: bool = true;

  fn invoke(&self, _: &mut druid::EventCtx, _: &mut T) {}
}

impl<T, F: Fn(&mut druid::EventCtx, &mut T)> OnCloseCallback<T> for F {
  const NOOP: bool = false;

  fn invoke(&self, ctx: &mut druid::EventCtx, data: &mut T) {
    (self)(ctx, data);
  }
}

impl<T> OnCloseCallback<T> for () {}

pub struct Modal<'a, T: Data, F: OnCloseCallback<T> = ()> {
  title: String,
  contents: Vec<StringOrWidget<'a, T>>,
  buttons: IndexMap<String, Vec<CommandOrFn<T>>>,
  on_close: F,
}

impl<'a, T: Data> Modal<'a, T> {
  pub fn new(title: &str) -> Self {
    Self {
      title: String::from(title),
      contents: Vec::new(),
      buttons: IndexMap::new(),
      on_close: (),
    }
  }

  pub fn with_content(mut self, content: impl Into<StringOrWidget<'a, T>>) -> Self {
    self.contents.push(content.into());

    self
  }

  pub fn with_button(mut self, label: &str, command: impl Into<CommandOrFn<T>>) -> Self {
    if let Some(commands) = self.buttons.get_mut(label) {
      commands.push(command.into());
    } else {
      self
        .buttons
        .insert(String::from(label), vec![command.into()]);
    }

    self
  }

  fn close(mut self, label: &str) -> Self {
    self.buttons.insert(String::from(label), Vec::new());

    self
  }

  pub fn with_close(self) -> Self {
    self.close("Close")
  }

  pub fn with_close_label(self, label: &str) -> Self {
    self.close(label)
  }

  pub fn with_on_close_override<F: Fn(&mut druid::EventCtx, &mut T) + 'static>(
    self,
    on_close: F,
  ) -> Modal<'a, T, F> {
    let Modal {
      title,
      contents,
      buttons,
      on_close: (),
    } = self;

    Modal {
      title,
      contents,
      buttons,
      on_close,
    }
  }
}

impl<'a, T: Data, F: OnCloseCallback<T> + 'static> Modal<'a, T, F> {
  pub fn build(mut self) -> impl Widget<T> {
    const CLOSE: Selector = Selector::new("modal.close");

    Flex::column()
      .with_child(
        h3_fixed(&self.title)
          .center()
          .padding(2.)
          .expand_width()
          .background(theme::BACKGROUND_LIGHT)
          .controller(DragWindowController::default()),
      )
      .with_flex_child(
        Stack::new()
          .with_child(
            Flex::column()
              .tap(|flex| {
                for content in self.contents.drain(..) {
                  flex.add_child(match content {
                    StringOrWidget::Str(str) => Label::wrapped(str).boxed(),
                    StringOrWidget::String(str) => Label::wrapped(str).boxed(),
                    StringOrWidget::Widget(widget) => widget,
                  });
                }
              })
              .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
              .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
              .padding((20., 5., 20., 20.))
              .scroll()
              .vertical()
              .expand(),
          )
          .with_positioned_child(
            Painter::new(|ctx, _, _| {
              let bounds = ctx.size().to_rect();
              let brush = LinearGradient::new(
                druid::UnitPoint::TOP,
                druid::UnitPoint::BOTTOM,
                (druid::Color::TRANSPARENT, druid::Color::BLACK),
              );
              ctx.fill(bounds, &brush);
            })
            .expand(),
            StackChildPosition::new()
              .bottom(Some(0.))
              .height(Some(20.))
              .left(Some(0.))
              .right(Some(0.)),
          )
          .expand(),
        1.,
      )
      .with_child(
        Flex::row()
          .with_flex_spacer(1.)
          .tap(|flex| {
            for (label, commands) in self.buttons.drain(..) {
              flex.add_child(Button::new(label).on_click({
                move |ctx, data, _| {
                  for command in &commands {
                    match command {
                      CommandOrFn::Command(command) => {
                        ctx.submit_command(command.clone().to(Target::Global));
                      }
                      CommandOrFn::Fn(func) => {
                        if let Some(func) = func.take() {
                          func(ctx, data);
                        }
                      }
                    }
                  }
                  ctx.submit_notification(CLOSE);
                }
              }));
            }
          })
          .on_notification(CLOSE, {
            let on_close = self.on_close;
            move |ctx, (), data| {
              if F::NOOP {
                ctx.submit_command(commands::CLOSE_WINDOW);
              } else {
                on_close.invoke(ctx, data);
              }
            }
          }),
      )
      .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
      .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
      .expand()
  }

  pub fn show(self, ctx: &mut (impl AnyCtx + RequestCtx), env: &Env, data: &T) -> WindowId {
    self.show_with_size(ctx, env, data, (500.0, 200.0))
  }

  pub fn show_with_size(
    self,
    ctx: &mut (impl AnyCtx + RequestCtx),
    env: &Env,
    data: &T,
    size: (f64, f64),
  ) -> WindowId {
    ctx.new_sub_window(
      WindowConfig::default()
        .show_titlebar(false)
        .resizable(true)
        .window_size(size),
      self.build(),
      data.clone(),
      env.clone(),
    )
  }
}

pub enum StringOrWidget<'a, T: Data> {
  Str(&'a str),
  String(String),
  Widget(Box<dyn Widget<T>>),
}

impl<'a, T: Data> From<&'a str> for StringOrWidget<'a, T> {
  fn from(str: &'a str) -> Self {
    StringOrWidget::Str(str)
  }
}

impl<'a, T: Data> From<String> for StringOrWidget<'a, T> {
  fn from(string: String) -> Self {
    StringOrWidget::String(string)
  }
}

impl<'a, T: Data> From<Box<dyn Widget<T>>> for StringOrWidget<'a, T> {
  fn from(widget: Box<dyn Widget<T>>) -> Self {
    StringOrWidget::Widget(widget)
  }
}

trait ModalButtonCallback<T> = FnOnce(&mut druid::EventCtx, &mut T);

pub enum CommandOrFn<T> {
  Command(Command),
  Fn(Cell<Option<Box<dyn ModalButtonCallback<T>>>>),
}

impl<T> From<Command> for CommandOrFn<T> {
  fn from(command: Command) -> Self {
    Self::Command(command)
  }
}

impl<T> From<Selector> for CommandOrFn<T> {
  fn from(selector: Selector) -> Self {
    Self::Command(selector.into())
  }
}

impl<T, F: FnOnce(&mut druid::EventCtx, &mut T) + 'static> From<F> for CommandOrFn<T> {
  fn from(func: F) -> Self {
    Self::Fn(Cell::new(Some(Box::new(func))))
  }
}

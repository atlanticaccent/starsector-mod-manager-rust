use druid::{
  commands, theme,
  widget::{Button, Flex, Label},
  Command, Data, Env, Target, Widget, WidgetExt, WindowConfig, WindowId,
};
use druid_widget_nursery::{AnyCtx, RequestCtx};
use indexmap::IndexMap;
use tap::Tap;

use super::util::{h3, DragWindowController, LabelExt};

pub struct Modal<'a, T: Data> {
  title: String,
  contents: Vec<StringOrWidget<'a, T>>,
  buttons: IndexMap<String, Vec<Command>>,
}

impl<'a, T: Data> Modal<'a, T> {
  pub fn new(title: &str) -> Self {
    Self {
      title: String::from(title),
      contents: Vec::new(),
      buttons: IndexMap::new(),
    }
  }

  pub fn with_content(mut self, content: impl Into<StringOrWidget<'a, T>>) -> Self {
    self.contents.push(content.into());

    self
  }

  pub fn with_button(mut self, label: &str, command: impl Into<Command>) -> Self {
    if let Some(commands) = self.buttons.get_mut(label) {
      commands.push(command.into())
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

  pub fn build(mut self) -> impl Widget<T> {
    Flex::column()
      .with_child(
        h3(&self.title)
          .center()
          .padding(2.)
          .expand_width()
          .background(theme::BACKGROUND_LIGHT)
          .controller(DragWindowController::default()),
      )
      .with_child(
        Flex::column()
          .tap_mut(|flex| {
            for content in self.contents.drain(..) {
              flex.add_child(match content {
                StringOrWidget::Str(str) => Label::wrapped(str).boxed(),
                StringOrWidget::String(str) => Label::wrapped(&str).boxed(),
                StringOrWidget::Widget(widget) => widget,
              })
            }
          })
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .expand_width()
          .padding(20.),
      )
      .with_flex_spacer(1.)
      .with_child(Flex::row().with_flex_spacer(1.).tap_mut(|flex| {
        for (label, commands) in self.buttons {
          flex.add_child(Button::new(label).on_click({
            let commands = commands.clone();
            move |ctx, _, _| {
              for command in &commands {
                ctx.submit_command(command.clone().to(Target::Global))
              }
              ctx.submit_command(commands::CLOSE_WINDOW)
            }
          }))
        }
      }))
      .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
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

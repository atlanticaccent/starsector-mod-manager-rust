use std::{fmt::Display, sync::Arc};

use druid::{
  im::Vector,
  theme,
  widget::{Container, Either, Flex, Scope, SizedBox},
  Color, Command, Data, Lens, Selector, Widget, WidgetExt as _,
};
use druid_widget_nursery::WidgetExt as _;

use crate::{
  app::{
    controllers::HoverController,
    util::{hoverable_text_opts, Compute, DummyTransfer, WidgetExtEx as _},
  },
  patch::tree::{Tree, TreeNode},
};

pub struct NavBar;

const FORCE_OPEN: Selector = Selector::new("force_open_tree");
const RECURSE_SET_EXPANDED: Selector<String> = Selector::new("recurse_set_expanded_tree");

impl NavBar {
  pub fn new<T: Data>(nav: Nav, default: &str) -> impl Widget<T> {
    Scope::from_function(
      move |_| nav,
      DummyTransfer::default(),
      NavBar::view(default),
    )
  }

  fn view(default: &str) -> impl Widget<Nav> {
    let default = default.to_owned();

    Tree::new(
      || {
        Either::new(
          |data, _| !data.separator_,
          Flex::row()
            .with_child(
              Container::new(SizedBox::empty().fix_size(6., 24.))
                .rounded(3.)
                .foreground(druid::theme::FOREGROUND_DARK)
                .env_scope(|env, data: &Nav| {
                  env.set(
                    druid::theme::FOREGROUND_DARK,
                    if data.expanded {
                      Color::GREEN
                    } else {
                      Color::TRANSPARENT
                    },
                  )
                })
                .padding((4., 0.)),
            )
            .with_child(
              hoverable_text_opts(None, |w| w.with_text_size(20.))
                .lens(Compute::new(|data: &Nav| data.label.clone()))
                .controller(HoverController)
                .on_click(|ctx, data, _| {
                  if !data.root {
                    ctx.submit_command(Nav::NAV_SELECTOR.with(data.label.clone()));
                    ctx.submit_command(
                      RECURSE_SET_EXPANDED
                        .with(data.linked.as_ref().unwrap_or(&data.label).clone()),
                    )
                  }
                }),
            )
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Center)
            .padding((4., 6.))
            .expand_width()
            .on_command(RECURSE_SET_EXPANDED, |ctx, label, data| {
              if data.root {
                data.set_ancestors_expanded(label);
              }
              ctx.set_handled()
            }),
          SizedBox::empty()
            .border(theme::BORDER_DARK, 1.)
            .padding((4., 6., 6., 0.))
            .expand_width(),
        )
        .or_empty(|data, _| !data.root)
      },
      Compute::new(|data: &Nav| data.expanded || data.always_open),
    )
    .with_opener(|| SizedBox::empty())
    .with_opener_dimensions((0., 0.))
    .with_max_label_height(26.)
    .with_indent(|data: &Nav| if data.depth > 1 { 16. } else { 0. })
    .on_command(FORCE_OPEN.clone(), |_, _, data| data.expanded = true)
    .on_added(move |_, ctx, _, _| ctx.submit_command(RECURSE_SET_EXPANDED.with(default.clone())))
  }
}

#[derive(Debug, Data, Clone, Lens)]
pub struct Nav {
  pub label: String,
  #[data(ignore)]
  pub command: Command,
  pub expanded: bool,
  pub children: Vector<Arc<Nav>>,
  pub root: bool,
  pub depth: usize,
  pub linked: Option<String>,
  pub separator_: bool,
  pub always_open: bool,
}

impl Nav {
  pub const NAV_SELECTOR: Selector<String> = Selector::new("nav_bar.switch");

  pub fn new(label: impl AsRef<str>) -> Self {
    Self {
      label: label.as_ref().to_string(),
      command: Nav::NAV_SELECTOR.with(label.as_ref().to_owned()),
      expanded: false,
      children: Vector::new(),
      root: false,
      depth: 0,
      linked: None,
      separator_: false,
      always_open: false,
    }
  }

  pub fn with_children(mut self, children: impl IntoIterator<Item = Nav>) -> Self {
    self.children = children
      .into_iter()
      .map(|mut nav| {
        nav.depth = self.depth + 1;
        nav.increment_children_depth();
        Arc::new(nav)
      })
      .collect();

    self
  }

  pub fn linked_to(mut self, label: impl Into<String>) -> Self {
    self.linked = Some(label.into());

    self
  }

  fn increment_children_depth(&mut self) {
    for child in self.children.iter_mut() {
      let mut child = Arc::make_mut(child);
      child.depth += 1;
      child.increment_children_depth()
    }
  }

  fn set_ancestors_expanded(&mut self, label: &str) -> bool {
    if self.label == label {
      self.expanded = true;
      return true;
    }

    let mut expand = false;
    for idx in 0..self.children_count() {
      self.for_child_mut(idx, |child: &mut Nav, _| {
        expand |= child.set_ancestors_expanded(label)
      });
    }

    self.expanded = expand;
    expand
  }

  pub fn as_root(mut self) -> Self {
    self.root = true;

    self
  }

  pub fn is_always_open(mut self) -> Self {
    self.always_open = true;

    self
  }

  pub fn separator() -> Self {
    Self {
      label: "".to_owned(),
      command: Selector::NOOP.with(()),
      expanded: false,
      children: Vector::new(),
      root: false,
      depth: 0,
      linked: None,
      separator_: true,
      always_open: false,
    }
  }
}

impl Display for Nav {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.label)
  }
}

impl TreeNode for Nav {
  fn children_count(&self) -> usize {
    self.children.len()
  }

  fn get_child(&self, index: usize) -> &Self {
    &self.children[index]
  }

  fn for_child_mut(&mut self, index: usize, mut cb: impl FnMut(&mut Self, usize)) {
    // Apply the closure to a clone of the child and update the `self.children` vector
    // with the clone iff it's changed to avoid unnecessary calls to `update(...)`

    // TODO: there must be a more idiomatic way to do this
    let orig = &self.children[index];
    let mut new = orig.as_ref().clone();
    cb(&mut new, index);
    if !orig.as_ref().same(&new) {
      self.children.remove(index);
      self.children.insert(index, Arc::new(new));
    }
  }
}

use std::{fmt::Display, sync::Arc};

use druid::{
  im::Vector,
  text::RichTextBuilder,
  theme,
  widget::{Container, Either, Flex, Label, Scope, SizedBox},
  Color, Command, Data, Lens, Selector, Widget, WidgetExt as _,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as _};

use crate::{
  app::{
    controllers::HoverController,
    util::{
      hoverable_text_opts, Compute, DummyTransfer, ShadeColor, WidgetExtEx as _, HOURGLASS_TOP,
    },
  },
  patch::tree::{Tree, TreeNode},
};

pub struct NavBar;

impl NavBar {
  pub const RECURSE_SET_EXPANDED: Selector<NavLabel> = Selector::new("recurse_set_expanded_tree");
  pub const SET_OVERRIDE: Selector<(NavLabel, bool)> = Selector::new("nav_bar.override");
  pub const REMOVE_OVERRIDE: Selector<NavLabel> = Selector::new("nav_bar.remove_override");

  pub fn new<T: Data>(nav: Nav, default: NavLabel) -> impl Widget<T> {
    Scope::from_function(
      move |_| nav,
      DummyTransfer::default(),
      NavBar::view(default),
    )
  }

  fn view(default: NavLabel) -> impl Widget<Nav> {
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
                    if data.expanded || data.override_.unwrap_or_default() {
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
                .lens(Compute::new(|data: &Nav| data.label.to_string()))
                .controller(HoverController::default())
                .on_click(|ctx, data, _| {
                  if !data.root {
                    ctx.submit_command(Nav::NAV_SELECTOR.with(data.linked.unwrap_or(data.label)));
                    ctx.submit_command(
                      NavBar::RECURSE_SET_EXPANDED.with(data.linked.unwrap_or(data.label)),
                    );
                  }
                })
                .else_if(
                  |data, _| data.label == NavLabel::Profiles,
                  Flex::row()
                    .with_child(Label::raw().with_text_size(20.).lens(Compute::new(
                      |data: &Nav| {
                        let mut builder = RichTextBuilder::new();
                        builder.push(&data.label.to_string()).strikethrough(true);

                        builder.build()
                      },
                    )))
                    .with_child(Icon::new(*HOURGLASS_TOP).with_color(druid::theme::DISABLED_TEXT_COLOR))
                    .env_scope(|env, _| {
                      env.set(
                        druid::theme::DISABLED_TEXT_COLOR,
                        env.get(druid::theme::DISABLED_TEXT_COLOR).darker_by(6),
                      )
                    })
                    .disabled(),
                ),
            )
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Center)
            .padding((4., 6.))
            .expand_width()
            .on_command(NavBar::RECURSE_SET_EXPANDED, |ctx, label, data| {
              if data.root {
                data.set_ancestors_expanded(*label);
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
      Compute::new(|data: &Nav| data.override_.unwrap_or(data.expanded || data.always_open)),
    )
    .with_opener(|| SizedBox::empty())
    .with_opener_dimensions((0., 0.))
    .with_max_label_height(26.)
    .with_indent(|data: &Nav| if data.depth > 1 { 16. } else { 0. })
    .on_added(move |_, ctx, _, _| ctx.submit_command(NavBar::RECURSE_SET_EXPANDED.with(default)))
    .on_command(Self::SET_OVERRIDE, |_, (label, override_), data| {
      data.set_override(*label, Some(*override_))
    })
    .on_command(Self::REMOVE_OVERRIDE, |_, target, data| {
      data.set_override(*target, None)
    })
  }
}

#[derive(Debug, Data, Clone, Lens)]
pub struct Nav {
  #[data(eq)]
  pub label: NavLabel,
  #[data(ignore)]
  pub command: Command,
  pub expanded: bool,
  pub children: Vector<Arc<Nav>>,
  pub root: bool,
  pub depth: usize,
  #[data(eq)]
  pub linked: Option<NavLabel>,
  pub separator_: bool,
  pub always_open: bool,
  pub override_: Option<bool>,
}

#[derive(strum_macros::Display, strum_macros::AsRefStr, Clone, Copy, PartialEq, Debug)]
#[strum(serialize_all = "title_case")]
pub enum NavLabel {
  Root,
  Mods,
  ModDetails,
  Profiles,
  Performance,
  ModBrowsers,
  Starmodder,
  StarmodderDetails,
  WebBrowser,
  Activity,
  Downloads,
  Settings,
  Separator,
}

impl From<NavLabel> for String {
  fn from(value: NavLabel) -> Self {
    value.to_string()
  }
}

impl Nav {
  pub const NAV_SELECTOR: Selector<NavLabel> = Selector::new("nav_bar.switch");

  pub fn new(label: NavLabel) -> Self {
    Self {
      label,
      command: Nav::NAV_SELECTOR.with(label),
      expanded: false,
      children: Vector::new(),
      root: false,
      depth: 0,
      linked: None,
      separator_: false,
      always_open: false,
      override_: None,
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

  pub fn linked_to(mut self, label: NavLabel) -> Self {
    self.linked = Some(label);

    self
  }

  fn increment_children_depth(&mut self) {
    for child in self.children.iter_mut() {
      let child = Arc::get_mut(child).unwrap();
      child.depth += 1;
      child.increment_children_depth()
    }
  }

  fn set_ancestors_expanded(&mut self, label: NavLabel) -> bool {
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
      label: NavLabel::Separator,
      command: Selector::NOOP.with(()),
      expanded: false,
      children: Vector::new(),
      root: false,
      depth: 0,
      linked: None,
      separator_: true,
      always_open: false,
      override_: None,
    }
  }

  pub fn overridden(mut self, override_: bool) -> Self {
    self.override_ = Some(override_);

    self
  }

  fn set_override(&mut self, target: NavLabel, override_: Option<bool>) {
    if self.label == target {
      self.override_ = override_;
      return;
    }
    for idx in 0..self.children_count() {
      self.for_child_mut(idx, |child, _| child.set_override(target, override_))
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
      self.children.set(index, Arc::new(new));
    }
  }
}

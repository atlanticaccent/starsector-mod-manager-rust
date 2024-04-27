use std::path::PathBuf;

use druid::{
  im::Vector,
  widget::{Flex, Label},
  Command, Data, Key, Lens, SingleUse, Widget, WidgetExt as _,
};
use druid_widget_nursery::{
  table::{FlexTable, TableColumnWidth, TableRow},
  wrap::Wrap,
};
use itertools::Itertools;
use tap::Pipe as _;

use crate::{
  app::{
    installer::{HybridPath, INSTALL_FOUND_MULTIPLE},
    mod_entry::ModEntry,
    util::{h2_fixed, ShadeColor, WidgetExtEx as _, BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
    App,
  },
  widgets::card::Card,
};

use super::Popup;

#[derive(Debug, Clone, Data)]
pub struct Multiple {
  pub source: HybridPath,
  found: Vector<ModEntry>,
}

#[derive(Debug, Clone, Data, Lens)]
struct MultipleState {
  enabled: Vector<bool>,
  #[data(ignore)]
  commands: Vec<Command>,
  #[data(ignore)]
  source: HybridPath,
}

impl Multiple {
  pub fn new(source: HybridPath, found: Vector<ModEntry>) -> Self {
    Self { source, found }
  }

  pub fn view(multiple: &Self) -> impl Widget<App> {
    let Self { source, found } = multiple.clone();
    let len = found.len();

    Card::builder()
      .with_insets(Card::CARD_INSET)
      .with_background(druid::theme::BACKGROUND_LIGHT)
      .build(
        Flex::column()
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .with_child(
            Wrap::new()
              .alignment(druid_widget_nursery::wrap::WrapAlignment::Center)
              .run_alignment(druid_widget_nursery::wrap::WrapAlignment::Center)
              .cross_alignment(druid_widget_nursery::wrap::WrapCrossAlignment::Center)
              .direction(druid::widget::Axis::Horizontal)
              .with_child(h2_fixed("Multiple mods found during installation from"))
              .with_child(h2_fixed(&source.source())),
          )
          .pipe(|column| {
            let mut column = column;

            let mut table = FlexTable::new()
              .with_column_width(TableColumnWidth::Flex(1.0))
              .with_column_width(TableColumnWidth::Intrinsic);
            for (idx, found) in found.iter().enumerate() {
              table.add_row(
                TableRow::new()
                  .with_child(row(found))
                  .with_child(install_button(found.path.clone(), source.clone(), idx)),
              )
            }

            column.add_child(table.scroll().vertical());

            column
          })
          .with_child(
            Flex::row()
              .with_child(
                Card::builder()
                  .with_insets((0.0, 8.0))
                  .with_corner_radius(6.0)
                  .with_shadow_length(2.0)
                  .with_shadow_increase(2.0)
                  .with_border(2.0, Key::new("button.border"))
                  .hoverable(|| {
                    Flex::row()
                      .with_child(Label::new("Install All").padding((10.0, 0.0)))
                      .align_vertical_centre()
                  })
                  .env_scope(|env, data: &MultipleState| {
                    let mut blue = env.get(BLUE_KEY);
                    let mut on_blue = env.get(ON_BLUE_KEY);

                    if data.enabled.all(false) {
                      blue = blue.darker_by(2);
                      on_blue = on_blue.darker_by(4);
                    }

                    env.set(druid::theme::BACKGROUND_LIGHT, blue);
                    env.set(druid::theme::TEXT_COLOR, on_blue);
                    env.set(Key::<druid::Color>::new("button.border"), on_blue);
                  })
                  .fix_height(42.0)
                  .padding((0.0, 2.0))
                  .on_click({
                    let source = source.clone();
                    move |ctx, data: &mut MultipleState, env| {
                      let installable = found
                        .iter()
                        .zip(data.enabled.iter())
                        .filter_map(|(entry, installable)| installable.then(|| entry.path.clone()))
                        .collect_vec();
                      dismiss(ctx, data, env);
                      ctx.submit_command(
                        INSTALL_FOUND_MULTIPLE.with(SingleUse::new((installable, source.clone()))),
                      );
                    }
                  })
                  .disabled_if(|data, _| data.enabled.all(false)),
              )
              .with_child(
                Card::builder()
                  .with_insets((0.0, 8.0))
                  .with_corner_radius(6.0)
                  .with_shadow_length(2.0)
                  .with_shadow_increase(2.0)
                  .with_border(2.0, Key::new("button.border"))
                  .hoverable(|| {
                    Flex::row()
                      .with_child(Label::new("Close").padding((10.0, 0.0)))
                      .align_vertical_centre()
                  })
                  .env_scope(|env, _| {
                    env.set(druid::theme::BACKGROUND_LIGHT, env.get(RED_KEY));
                    env.set(druid::theme::TEXT_COLOR, env.get(ON_RED_KEY));
                    env.set(
                      Key::<druid::Color>::new("button.border"),
                      env.get(ON_RED_KEY),
                    );
                  })
                  .fix_height(42.0)
                  .padding((0.0, 2.0))
                  .on_click(dismiss),
              )
              .align_right(),
          ),
      )
      .scope_independent({
        let source = source.clone();
        move || MultipleState {
          enabled: Vector::from(vec![true; len]),
          commands: vec![],
          source: source.clone(),
        }
      })
  }
}

fn dismiss(ctx: &mut druid::EventCtx, data: &mut MultipleState, _env: &druid::Env) {
  ctx.submit_command(Popup::DISMISS);
  for command in data.commands.drain(0..) {
    ctx.submit_command(command)
  }
}

fn row<T: Data>(entry: &ModEntry) -> impl Widget<T> {
  FlexTable::new()
    .with_column_width((TableColumnWidth::Intrinsic, TableColumnWidth::Flex(0.1)))
    .with_column_width(TableColumnWidth::Flex(9.9))
    .with_row(
      TableRow::new()
        .with_child(Label::new("Name:"))
        .with_child(Label::new(entry.name.clone())),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("ID:"))
        .with_child(Label::new(entry.id.clone())),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("Version:"))
        .with_child(Label::new(entry.version.to_string())),
    )
    .with_row(
      TableRow::new()
        .with_child(Label::new("Path:"))
        .with_child(Label::new(entry.path.to_string_lossy().as_ref())),
    )
}

fn install_button(path: PathBuf, source: HybridPath, idx: usize) -> impl Widget<MultipleState> {
  Card::builder()
    .with_insets((0.0, 8.0))
    .with_corner_radius(6.0)
    .with_shadow_length(2.0)
    .with_shadow_increase(2.0)
    .with_border(2.0, Key::new("button.border"))
    .hoverable(|| {
      Flex::row()
        .with_child(Label::new("Install").padding((10.0, 0.0)))
        .align_vertical_centre()
    })
    .env_scope(move |env, data: &MultipleState| {
      let mut blue = env.get(BLUE_KEY);
      let mut on_blue = env.get(ON_BLUE_KEY);

      if !data.enabled[idx] {
        blue = blue.darker_by(2);
        on_blue = on_blue.darker_by(4);
      }

      env.set(druid::theme::BACKGROUND_LIGHT, blue);
      env.set(druid::theme::TEXT_COLOR, on_blue);
      env.set(Key::<druid::Color>::new("button.border"), on_blue);
    })
    .fix_height(42.0)
    .padding((0.0, 2.0))
    .on_click(move |ctx, state: &mut MultipleState, env| {
      let can_install = &mut state.enabled[idx];
      if *can_install {
        ctx.clear_cursor();
        ctx.set_active(false);
        if ctx.is_focused() {
          ctx.resign_focus()
        }
        *can_install = false;
        state
          .commands
          .push(INSTALL_FOUND_MULTIPLE.with(SingleUse::new((vec![path.clone()], source.clone()))));
        if state.enabled.all(false) {
          dismiss(ctx, state, env)
        }
      }
    })
    .disabled_if(move |data, _| !data.enabled[idx])
}

#[extend::ext]
impl Vector<bool> {
  fn any(&self, matches: bool) -> bool {
    self.iter().any(|v| *v == matches)
  }

  fn all(&self, matches: bool) -> bool {
    self.iter().all(|v| *v == matches)
  }
}

use std::path::PathBuf;

use druid::{
  im::Vector,
  widget::{Flex, Label},
  Command, Data, Key, Lens, SingleUse, Widget, WidgetExt as _,
};
use druid_widget_nursery::wrap::Wrap;
use itertools::Itertools;

use super::Popup;
use crate::{
  app::{
    installer::{HybridPath, INSTALL_FOUND_MULTIPLE},
    mod_entry::ModEntry,
    util::{bolded, h2_fixed, hoverable_text, ShadeColor, Tap as _, WidgetExtEx as _},
    App,
  },
  patch::table::{FixedFlexTable, TableColumnWidth, TableRow},
  theme::{BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
  widgets::card::Card,
};

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

  pub fn view(&self) -> impl Widget<App> {
    let Self { source, found } = self.clone();
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

            let mut table = FixedFlexTable::new()
              .with_column_width(TableColumnWidth::Flex(1.0))
              .with_column_width(TableColumnWidth::Intrinsic);
            for (idx, found) in found.iter().enumerate() {
              table.add_row(
                TableRow::new()
                  .with_child(row(found))
                  .with_child(install_button(found.path.clone(), source.clone(), idx)),
              );
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
                  .hoverable(|_| {
                    Flex::row()
                      .with_child(Label::new("Install All").padding((10.0, 0.0)))
                      .valign_centre()
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
                        .filter(|&(_, installable)| *installable)
                        .map(|(entry, _)| entry.path.clone())
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
                  .hoverable(|_| {
                    Flex::row()
                      .with_child(Label::new("Close").padding((10.0, 0.0)))
                      .valign_centre()
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
    ctx.submit_command(command);
  }
}

#[allow(irrefutable_let_patterns)]
fn row<T: Data>(entry: &ModEntry) -> impl Widget<T> {
  let path = entry.path.clone();
  FixedFlexTable::new()
    .with_column_width((TableColumnWidth::Intrinsic, TableColumnWidth::Flex(0.5)))
    .with_column_width(TableColumnWidth::Flex(9.5))
    .with_row(
      TableRow::new()
        .with_child(bolded("Name:").align_right())
        .with_child(Label::new(entry.name.clone())),
    )
    .with_row(
      TableRow::new()
        .with_child(bolded("ID:").align_right())
        .with_child(Label::new(entry.id.clone())),
    )
    .with_row(
      TableRow::new()
        .with_child(bolded("Version:").align_right())
        .with_child(Label::new(
          if let val = entry.version.to_string()
            && !val.is_empty()
          {
            val
          } else {
            "Version not specified".to_owned()
          },
        )),
    )
    .with_row(
      TableRow::new()
        .with_child(bolded("Path:").align_right())
        .with_child(
          hoverable_text(Option::<druid::Color>::None)
            .constant(entry.path.to_string_lossy().to_string())
            .on_click(move |ctx, _, _| {
              let _ = opener::open(path.clone());
              ctx.set_active(false);
              ctx.clear_cursor();
              if ctx.is_focused() {
                ctx.resign_focus();
              }
              ctx.request_update();
            }),
        ),
    )
}

fn install_button(path: PathBuf, source: HybridPath, idx: usize) -> impl Widget<MultipleState> {
  Card::builder()
    .with_insets((0.0, 8.0))
    .with_corner_radius(6.0)
    .with_shadow_length(2.0)
    .with_shadow_increase(2.0)
    .with_border(2.0, Key::new("button.border"))
    .hoverable(|_| {
      Flex::row()
        .with_child(Label::new("Install").padding((10.0, 0.0)))
        .valign_centre()
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
          ctx.resign_focus();
        }
        *can_install = false;
        state
          .commands
          .push(INSTALL_FOUND_MULTIPLE.with(SingleUse::new((vec![path.clone()], source.clone()))));
        if state.enabled.all(false) {
          dismiss(ctx, state, env);
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

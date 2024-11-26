use common::{
  labels::{bolded, h2_fixed, hoverable_text},
  theme_keys::{GREEN_KEY, ON_GREEN_KEY},
  widget_ext::WidgetExtEx,
  widgets::card::Card,
};
use druid::{
  im::Vector,
  widget::{Flex, Label, Painter, SizedBox},
  Data, Key, Lens, Selector, SingleUse, Widget, WidgetExt as _,
};
use druid_patch::table::{FixedFlexTable, TableColumnWidth, TableRow};
use druid_widget_nursery::wrap::Wrap;
use installer::HybridPath;

use super::Popup;
use crate::{
  app::{
    installer_impl::{InstallMessage, INSTALL},
    mod_entry::ModEntry,
    App,
  },
  theme::{BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
};

#[derive(Clone, Data)]
pub struct Multiple {
  #[data(eq)]
  pub source: HybridPath,
  found: Vector<ModEntry>,
}

#[derive(Debug, Clone, Data, Lens)]
struct MultipleState {
  selected: Vector<bool>,
}

impl Multiple {
  pub fn new(source: HybridPath, found: Vector<ModEntry>) -> Self {
    Self { source, found }
  }

  pub fn view(&self) -> impl Widget<App> {
    const DISMISS_SELF: Selector<bool> = Selector::new("found_multiple.install");

    let Self { source, found } = self.clone();
    let len = found.len();

    let body = Card::builder()
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
          .with_spacer(5.0)
          .with_child({
            let mut table = FixedFlexTable::new()
              .with_column_width(TableColumnWidth::Flex(1.0))
              .with_column_width(TableColumnWidth::Intrinsic)
              .row_background(Painter::new(move |ctx, _, env| {
                use druid::RenderContext;
                let rect = ctx.size().to_rect();

                if env
                  .try_get(FixedFlexTable::<MultipleState>::ROW_IDX)
                  .unwrap_or(0)
                  % 3
                  == 0
                {
                  ctx.fill(rect, &env.get(druid::theme::BACKGROUND_DARK));
                } else {
                  ctx.fill(rect, &env.get(druid::theme::BACKGROUND_LIGHT));
                }
              }));
            for (idx, found) in found.iter().enumerate() {
              table.add_row(
                TableRow::new()
                  .with_child(row(found))
                  .with_child(install_button(idx).align_right()),
              );
              if idx < len - 1 {
                table.add_row(
                  TableRow::new()
                    .with_child(SizedBox::empty().fix_height(5.0))
                    .with_child(SizedBox::empty()),
                );
              }
            }

            table.scroll().vertical()
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
                  .env_scope(|env, _: &MultipleState| {
                    let bg = env.get(BLUE_KEY);
                    let text = env.get(ON_BLUE_KEY);

                    env.set(druid::theme::BACKGROUND_LIGHT, bg);
                    env.set(druid::theme::TEXT_COLOR, text);
                    env.set(Key::<druid::Color>::new("button.border"), text);
                  })
                  .fix_height(42.0)
                  .padding((0.0, 2.0))
                  .on_click(|ctx, _, _| {
                    ctx.submit_notification(DISMISS_SELF.with(true));
                  }),
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
                      .with_child(Label::new("Install Selected").padding((10.0, 0.0)))
                      .valign_centre()
                  })
                  .env_scope(|env, _: &MultipleState| {
                    let bg = env.get(GREEN_KEY);
                    let text = env.get(ON_GREEN_KEY);

                    env.set(druid::theme::BACKGROUND_LIGHT, bg);
                    env.set(druid::theme::TEXT_COLOR, text);
                    env.set(Key::<druid::Color>::new("button.border"), text);
                  })
                  .fix_height(42.0)
                  .padding((0.0, 2.0))
                  .on_click(|ctx, _, _| {
                    ctx.submit_notification(DISMISS_SELF.with(false));
                  })
                  .empty_if(|data, _| data.selected.all(false)),
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
                      .with_child(Label::new("Cancel").padding((10.0, 0.0)))
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
                  .on_click(|ctx, _, _| {
                    ctx.submit_command(Popup::DISMISS);
                  }),
              )
              .align_right(),
          ),
      )
      .on_notification(DISMISS_SELF, {
        let multiple = SingleUse::new((source, found));
        move |ctx, install_all, data| {
          ctx.submit_command(Popup::DISMISS);

          let Some((source, found)) = multiple.take() else {
            return;
          };
          let targets = found
            .into_iter()
            .zip(&data.selected)
            .filter_map(|(target, selected)| (*install_all || *selected).then_some(target.path))
            .collect();
          ctx.submit_command(INSTALL.with(SingleUse::new(InstallMessage::FoundMultiple(
            targets, source,
          ))));
        }
      })
      .scope_independent(move || MultipleState {
        selected: Vector::from(vec![false; len]),
      });

    Flex::row()
      .must_fill_main_axis(true)
      .with_flex_spacer(0.5)
      .with_flex_child(body, 1.0)
      .with_flex_spacer(0.5)
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

fn install_button(idx: usize) -> impl Widget<MultipleState> {
  Card::builder()
    .with_insets((0.0, 8.0))
    .with_corner_radius(6.0)
    .with_shadow_length(2.0)
    .with_shadow_increase(2.0)
    .with_border(2.0, Key::new("button.border"))
    .hoverable(move |_| {
      Flex::row()
        .with_child(
          Label::new("Select")
            .else_if(
              move |data: &MultipleState, _| data.selected[idx],
              Label::new("Selected"),
            )
            .padding((10.0, 0.0)),
        )
        .valign_centre()
    })
    .env_scope(move |env, data: &MultipleState| {
      let (bg, text) = if data.selected[idx] {
        (env.get(GREEN_KEY), env.get(ON_GREEN_KEY))
      } else {
        (env.get(BLUE_KEY), env.get(ON_BLUE_KEY))
      };

      env.set(druid::theme::BACKGROUND_LIGHT, bg);
      env.set(druid::theme::TEXT_COLOR, text);
      env.set(Key::<druid::Color>::new("button.border"), text);
    })
    .fix_height(42.0)
    .padding((0.0, 2.0))
    .on_click(move |ctx, state: &mut MultipleState, _| {
      let selected = &mut state.selected[idx];
      *selected = !*selected;
      if *selected {
        ctx.clear_cursor();
        ctx.set_active(false);
        if ctx.is_focused() {
          ctx.resign_focus();
        }
      }
    })
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

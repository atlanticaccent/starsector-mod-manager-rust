use std::marker::PhantomData;

use druid::{
  kurbo::Line,
  text::{FontWeight, RichTextBuilder},
  widget::{Either, Flex, Label, Painter, RawLabel},
  Data, Key, Widget, WidgetExt,
};
use druid_widget_nursery::material_icons::Icon;
use self_update::cargo_crate_version;

use crate::{
  app::{
    controllers::HoverController,
    overlays::Popup,
    updater::{CopyTx, Release},
    util::{h2_fixed, hyperlink_opts, LabelExt, WidgetExtEx, ARROW_DROP_DOWN, ARROW_RIGHT},
    App,
  },
  theme::{BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
  widgets::card::Card,
};

#[derive(Debug, Clone, Data)]
pub enum Status {
  Ready(Release, CopyTx),
  Completed,
  CheckFailed(String),
  InstallFailed(String),
}

impl Status {
  pub fn view<T: Data>(&self) -> impl Widget<T> {
    match self {
      Status::Ready(release, tx) => View::prompt_update(release, tx).boxed(),
      Status::CheckFailed(error) => View::check_failed(error).boxed(),
      Status::Completed => View::success().boxed(),
      Status::InstallFailed(error) => View::install_failed(error).boxed(),
    }
  }
}

struct View<T>(PhantomData<T>);

impl<T: Data> View<T> {
  fn prompt_update(release: &Release, tx: &CopyTx) -> impl Widget<T> {
    Flex::row()
      .must_fill_main_axis(true)
      .with_flex_spacer(0.5)
      .with_flex_child(
        Card::builder()
          .with_insets(Card::CARD_INSET)
          .with_background(druid::theme::BACKGROUND_LIGHT)
          .build(
            Flex::column()
              .with_child(h2_fixed("An update is available for MOSS."))
              .with_child({
                let mut builder = RichTextBuilder::new();
                builder
                  .push("Current:")
                  .weight(FontWeight::MEDIUM)
                  .underline(true);
                builder.push("  ");
                builder.push(cargo_crate_version!());

                RawLabel::new().constant(builder.build())
              })
              .with_child({
                let mut builder = RichTextBuilder::new();
                builder
                  .push("Newest:")
                  .weight(FontWeight::MEDIUM)
                  .underline(true);
                builder.push("  ");
                builder.push(&release.version);

                RawLabel::new().constant(builder.build())
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
                          .with_child(Label::new("Install").padding((10.0, 0.0)))
                          .valign_centre()
                      })
                      .env_scope(|env, _| {
                        env.set(druid::theme::BACKGROUND_LIGHT, env.get(BLUE_KEY));
                        env.set(druid::theme::TEXT_COLOR, env.get(ON_BLUE_KEY));
                        env.set(
                          Key::<druid::Color>::new("button.border"),
                          env.get(ON_BLUE_KEY),
                        );
                      })
                      .fix_height(42.0)
                      .padding((0.0, 2.0))
                      .on_click({
                        let tx = tx.clone();
                        move |ctx, _, _| {
                          ctx.submit_command(Popup::DISMISS);
                          tx.send(true);
                        }
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
                      .on_click({
                        let tx = tx.clone();
                        move |ctx, _, _| {
                          ctx.submit_command(Popup::DISMISS);
                          tx.send(false);
                        }
                      }),
                  ),
              )
              .expand_width(),
          ),
        1.0,
      )
      .with_flex_spacer(0.5)
  }

  fn check_failed(error: &str) -> impl Widget<T> {
    Flex::row()
      .must_fill_main_axis(true)
      .with_flex_spacer(0.5)
      .with_flex_child(
        Card::builder()
          .with_insets(Card::CARD_INSET)
          .with_background(druid::theme::BACKGROUND_LIGHT)
          .build(
            Flex::column()
              .with_child(h2_fixed("Could not retrieve updates."))
              .with_child(Label::new("There may be updates available."))
              .with_spacer(5.0)
              .with_child(
                Flex::row()
                  .with_child(
                    Flex::row()
                      .with_child(Either::new(
                        |data, _| *data,
                        Icon::new(*ARROW_DROP_DOWN).with_color(ON_RED_KEY),
                        Icon::new(*ARROW_RIGHT).with_color(ON_RED_KEY),
                      ))
                      .with_child(Label::new("Error:").with_text_color(ON_RED_KEY))
                      .main_axis_alignment(druid::widget::MainAxisAlignment::End)
                      .align_right()
                      .controller(HoverController::default())
                      .on_click(|_, data, _| *data = !*data)
                      .padding((0., -2., 0., 0.)),
                  )
                  .with_spacer(2.0)
                  .with_flex_child(
                    Either::new(
                      |data, _| *data,
                      Label::wrapped(error).with_text_color(ON_RED_KEY),
                      Label::new(error)
                        .with_text_color(ON_RED_KEY)
                        .with_line_break_mode(druid::widget::LineBreaking::Clip)
                        .controller(HoverController::default())
                        .on_click(|_, data: &mut bool, _| *data = !*data),
                    )
                    .padding((2.0, 0.0))
                    .foreground(Painter::new(|ctx, _, env| {
                      use druid::RenderContext;
                      let size = ctx.size();
                      ctx.stroke(
                        Line::new((0.0, 0.0), (0.0, size.height)),
                        &env.get(ON_RED_KEY),
                        1.0,
                      );
                    })),
                    1.0,
                  )
                  .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
                  .scope_independent(|| false)
                  .padding(2.0)
                  .background(RED_KEY)
                  .rounded(2.0)
                  .border(ON_RED_KEY, 2.0),
              )
              .with_spacer(5.0)
              .with_child(Label::wrapped(
                "The latest releases for MOSS can be found here:",
              ))
              .with_child(hyperlink_opts(App::OPEN_EXTERNALLY).constant(
                "https://github.com/atlanticaccent/starsector-mod-manager-rust/releases/latest",
              ))
              .with_child(
                Flex::row().with_child(
                  Card::builder()
                    .with_insets((0.0, 8.0))
                    .with_corner_radius(6.0)
                    .with_shadow_length(2.0)
                    .with_shadow_increase(2.0)
                    .with_border(2.0, Key::new("button.border"))
                    .hoverable(|_| {
                      Flex::row()
                        .with_child(Label::new("Dismiss").padding((10.0, 0.0)))
                        .valign_centre()
                    })
                    .env_scope(|env, _| {
                      env.set(druid::theme::BACKGROUND_LIGHT, env.get(BLUE_KEY));
                      env.set(druid::theme::TEXT_COLOR, env.get(ON_BLUE_KEY));
                      env.set(
                        Key::<druid::Color>::new("button.border"),
                        env.get(ON_BLUE_KEY),
                      );
                    })
                    .fix_height(42.0)
                    .padding((0.0, 2.0))
                    .on_click(move |ctx, _, _| {
                      ctx.submit_command(Popup::DISMISS);
                    }),
                ),
              )
              .expand_width(),
          ),
        1.0,
      )
      .with_flex_spacer(0.5)
      .boxed()
  }

  fn success() -> impl Widget<T> {
    Flex::row()
      .must_fill_main_axis(true)
      .with_flex_spacer(0.5)
      .with_flex_child(
        Card::builder()
          .with_insets(Card::CARD_INSET)
          .with_background(druid::theme::BACKGROUND_LIGHT)
          .build(
            Flex::column()
              .with_child(h2_fixed("Update Complete. Restart?"))
              .with_child(Label::wrapped(
                "MOSS needs to restart to finish applying updates.",
              ))
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
                          .with_child(Label::new("Restart now").padding((10.0, 0.0)))
                          .valign_centre()
                      })
                      .env_scope(|env, _| {
                        env.set(druid::theme::BACKGROUND_LIGHT, env.get(BLUE_KEY));
                        env.set(druid::theme::TEXT_COLOR, env.get(ON_BLUE_KEY));
                        env.set(
                          Key::<druid::Color>::new("button.border"),
                          env.get(ON_BLUE_KEY),
                        );
                      })
                      .fix_height(42.0)
                      .padding((0.0, 2.0))
                      .on_click(move |ctx, _, _| {
                        ctx.submit_command(Popup::DISMISS);
                        ctx.submit_command(App::RESTART);
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
                      .on_click(move |ctx, _, _| {
                        ctx.submit_command(Popup::DISMISS);
                      }),
                  ),
              )
              .expand_width(),
          ),
        1.0,
      )
      .with_flex_spacer(0.5)
      .boxed()
  }

  fn install_failed(error: &str) -> impl Widget<T> {
    Flex::row()
      .must_fill_main_axis(true)
      .with_flex_spacer(0.5)
      .with_flex_child(
        Card::builder()
          .with_insets(Card::CARD_INSET)
          .with_background(druid::theme::BACKGROUND_LIGHT)
          .build(
            Flex::column()
              .with_child(h2_fixed("Failed to install update."))
              .with_spacer(5.0)
              .with_child(
                Flex::row()
                  .with_child(
                    Flex::row()
                      .with_child(Either::new(
                        |data, _| *data,
                        Icon::new(*ARROW_DROP_DOWN).with_color(ON_RED_KEY),
                        Icon::new(*ARROW_RIGHT).with_color(ON_RED_KEY),
                      ))
                      .with_child(Label::new("Error:").with_text_color(ON_RED_KEY))
                      .main_axis_alignment(druid::widget::MainAxisAlignment::End)
                      .align_right()
                      .controller(HoverController::default())
                      .on_click(|_, data, _| *data = !*data)
                      .padding((0., -2., 0., 0.)),
                  )
                  .with_spacer(2.0)
                  .with_flex_child(
                    Either::new(
                      |data, _| *data,
                      Label::wrapped(error).with_text_color(ON_RED_KEY),
                      Label::new(error)
                        .with_text_color(ON_RED_KEY)
                        .with_line_break_mode(druid::widget::LineBreaking::Clip)
                        .controller(HoverController::default())
                        .on_click(|_, data: &mut bool, _| *data = !*data),
                    )
                    .padding((2.0, 0.0))
                    .foreground(Painter::new(|ctx, _, env| {
                      use druid::RenderContext;
                      let size = ctx.size();
                      ctx.stroke(
                        Line::new((0.0, 0.0), (0.0, size.height)),
                        &env.get(ON_RED_KEY),
                        1.0,
                      );
                    })),
                    1.0,
                  )
                  .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
                  .scope_independent(|| false)
                  .padding(2.0)
                  .background(RED_KEY)
                  .rounded(2.0)
                  .border(ON_RED_KEY, 2.0),
              )
              .with_spacer(5.0)
              .with_child(
                Flex::row().with_child(
                  Card::builder()
                    .with_insets((0.0, 8.0))
                    .with_corner_radius(6.0)
                    .with_shadow_length(2.0)
                    .with_shadow_increase(2.0)
                    .with_border(2.0, Key::new("button.border"))
                    .hoverable(|_| {
                      Flex::row()
                        .with_child(Label::new("Dismiss").padding((10.0, 0.0)))
                        .valign_centre()
                    })
                    .env_scope(|env, _| {
                      env.set(druid::theme::BACKGROUND_LIGHT, env.get(BLUE_KEY));
                      env.set(druid::theme::TEXT_COLOR, env.get(ON_BLUE_KEY));
                      env.set(
                        Key::<druid::Color>::new("button.border"),
                        env.get(ON_BLUE_KEY),
                      );
                    })
                    .fix_height(42.0)
                    .padding((0.0, 2.0))
                    .on_click(move |ctx, _, _| {
                      ctx.submit_command(Popup::DISMISS);
                    }),
                ),
              )
              .expand_width(),
          ),
        1.0,
      )
      .with_flex_spacer(0.5)
      .boxed()
  }
}

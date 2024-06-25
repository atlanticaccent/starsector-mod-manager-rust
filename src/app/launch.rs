use std::path::PathBuf;

use druid::{
  lens::Map,
  widget::{Container, Flex, Label},
  Color, Data, ExtEventSink, Key, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as _};
use futures::TryFutureExt;
use webview_shared::ExtEventSinkExt;

use super::{
  overlays::{LaunchResult, Popup},
  util::{State, WithHoverState},
  App,
};
use crate::{
  app::{
    util::{bold_text, ShadeColor, WidgetExtEx}, ARROW_DROP_DOWN, CHEVRON_LEFT, INFO, PLAY_ARROW
  },
  patch::separator::Separator,
  widgets::{
    card::Card,
    card_button::{CardButton, ScopedStackCardButton},
  },
};

const OLD_TEXT_COLOR: druid::Key<druid::Color> = druid::Key::new("old_text_colour");
const OVERRIDE_HOVER: Selector<bool> = Selector::new("launch.dropdown.collapsed.hovered.supp");
const OPEN_STACK: Selector = Selector::new("launch_button.stack.open");

fn text_maker<T: Data>(text: &str) -> impl Widget<T> {
  bold_text(
    text,
    druid::theme::TEXT_SIZE_LARGE,
    druid::FontWeight::SEMI_BOLD,
    druid::theme::TEXT_COLOR,
  )
}

pub(crate) fn launch_button() -> impl Widget<App> {
  let light_gray = druid::Color::GRAY.lighter_by(2);

  Card::builder()
    .with_background(druid::Color::BLACK.interpolate_with(druid::Color::GRAY, 1))
    .with_border(1.0, light_gray)
    .stacked_button(
      |_| {
        Flex::column()
          .with_child(launch_button_body())
          .with_child(footer_collapsed())
          .env_scope(move |env, _| {
            env.set(OLD_TEXT_COLOR, env.get(druid::theme::TEXT_COLOR));
            env.set(druid::theme::TEXT_COLOR, druid::Color::WHITE.darker())
          })
      },
      |_| {
        Flex::column()
          .with_child(launch_button_body())
          .with_child(footer_expanded())
          .env_scope(move |env, _| {
            env.set(OLD_TEXT_COLOR, env.get(druid::theme::TEXT_COLOR));
            env.set(druid::theme::TEXT_COLOR, druid::Color::WHITE.darker())
          })
          .expand_width()
          .on_click(|ctx, _, _| ctx.submit_command(crate::widgets::root_stack::RootStack::DISMISS))
      },
      Some(
        |widget: ScopedStackCardButton<App>,
         dropdown: std::rc::Rc<dyn Fn() -> Box<dyn Widget<App>>>,
         id| {
          widget.on_command(OPEN_STACK, move |ctx, _, data| {
            CardButton::trigger_dropdown_manually(ctx, dropdown.clone(), id, data)
          })
        },
      ),
      195.0,
    )
    .on_event(|_, ctx, event, _| {
      if let druid::Event::MouseMove(mouse) = event {
        let bottom_y = ctx.size().height - mouse.pos.y;
        if bottom_y >= 7.0 && bottom_y <= 14.0 {
          ctx.submit_command(OVERRIDE_HOVER.with(true));
          ctx.clear_cursor();
          ctx.override_cursor(&druid::Cursor::Arrow);
        } else {
          if bottom_y < 7.0 {
            ctx.submit_command(OVERRIDE_HOVER.with(false))
          }
          ctx.clear_cursor()
        }
      }

      false
    })
    .on_click2(|ctx, mouse, _, _| {
      let bottom_y = ctx.size().height - mouse.pos.y;
      if bottom_y >= 7.0 && bottom_y <= 14.0 {
        ctx.submit_command(OPEN_STACK)
      }
    })
    .expand_width()
}

fn footer_collapsed() -> impl Widget<App> {
  const BACKGROUND: Key<Color> = Key::new("launch.button.dropdown.collapsed.hovered");
  let light_gray = druid::Color::GRAY.lighter_by(2);

  let child = Flex::column()
    .with_child(Separator::new().with_color(light_gray).with_width(1.0))
    .with_default_spacer()
    .with_child(
      Flex::row()
        .with_default_spacer()
        .with_child(Label::new("Official Launcher").else_if(
          |data: &App, _| data.settings.experimental_launch,
          experimental_launch_row(),
        ))
        .with_flex_spacer(1.0)
        .with_child(Icon::new(*CHEVRON_LEFT).fix_size(20.0, 20.0)),
    )
    .with_default_spacer()
    .background(BACKGROUND)
    .padding((1.0, 0.0, 1.0, 0.0));

  Container::new(child)
    .rounded(4.0)
    .padding((0.0, 0.0, 0.0, -6.0))
    .scope_with(false, |widget| {
      widget
        .env_scope(move |env, state| {
          if state.inner {
            env.set(BACKGROUND, Color::GRAY.lighter_by(4));
            env.set(druid::theme::TEXT_COLOR, env.get(OLD_TEXT_COLOR));
          } else {
            env.set(BACKGROUND, Color::BLACK.interpolate_with(Color::GRAY, 1))
          };
        })
        .lens(Map::new(
          |data: &(State<App, bool>, bool)| {
            let mut out = data.0.clone();
            out.inner = data.1;
            out
          },
          |data, state| data.0 = state,
        ))
        .on_event(|_, ctx, _, data| {
          if ctx.is_hot() || data.0.inner || data.1 {
            ctx.clear_cursor();
            ctx.set_cursor(&druid::Cursor::Arrow);
            ctx.override_cursor(&druid::Cursor::Arrow);
          }

          false
        })
        .on_command(OVERRIDE_HOVER, |ctx, payload, data| {
          data.1 = *payload;
          data.0.inner = *payload;
          ctx.request_update();
          ctx.request_paint();
        })
        .with_hover_state(false)
    })
    .on_click(|ctx, _, _| ctx.submit_command(OPEN_STACK))
}

fn footer_expanded() -> impl Widget<App> {
  const BACKGROUND: Key<Color> = Key::new("launch.button.dropdown.collapsed.hovered");
  let light_gray = druid::Color::GRAY.lighter_by(2);

  let child = Flex::column()
    .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
    .with_child(Separator::new().with_color(light_gray).with_width(1.0))
    .with_default_spacer()
    .with_child(
      Flex::row()
        .with_default_spacer()
        .with_child(Label::new("Official Launcher").else_if(
          |data: &App, _| data.settings.experimental_launch,
          experimental_launch_row(),
        ))
        .with_flex_spacer(1.0)
        .with_child(Icon::new(*ARROW_DROP_DOWN).fix_size(20.0, 20.0)),
    )
    .with_default_spacer()
    .with_child(
      Flex::row()
        .with_default_spacer()
        .with_child(Label::new("Official Launcher").else_if(
          |data: &App, _| !data.settings.experimental_launch,
          experimental_launch_row(),
        ))
        .with_flex_spacer(1.0)
        .padding((0.0, 10.0))
        .background(BACKGROUND)
        .rounded(6.0)
        .wrap_with_hover_state(false, false, |scoped| {
          scoped.env_scope(|env, data| {
            env.set(
              BACKGROUND,
              if data.1 {
                Color::GRAY.lighter_by(8)
              } else {
                Color::GRAY.lighter_by(4)
              },
            )
          })
        })
        .on_click(|_, data, _| {
          data.settings.experimental_launch = !data.settings.experimental_launch
        }),
    )
    .background(Color::GRAY.lighter_by(4))
    .padding((1.0, 0.0, 1.0, 0.0));

  Container::new(child)
    .rounded(4.0)
    .padding((0.0, 0.0, 0.0, -6.0))
    .scope_with(false, |widget| {
      widget
        .env_scope(move |env, state| {
          env.set(druid::theme::TEXT_COLOR, env.get(OLD_TEXT_COLOR));
        })
        .lens(Map::new(
          |data: &(State<App, bool>, bool)| {
            let mut out = data.0.clone();
            out.inner = data.1;
            out
          },
          |data, state| data.0 = state,
        ))
        .on_event(|_, ctx, _, data| {
          if ctx.is_hot() || data.0.inner || data.1 {
            ctx.clear_cursor();
            ctx.set_cursor(&druid::Cursor::Arrow);
            ctx.override_cursor(&druid::Cursor::Arrow);
          }

          false
        })
        .with_hover_state(false)
    })
}

fn experimental_launch_row() -> Flex<App> {
  Flex::row()
    .with_child(Label::new("Directly"))
    .with_spacer(2.5)
    .with_child(
      Icon::new(*INFO)
        .fix_size(20.0, 20.0)
        .align_vertical(druid::UnitPoint::TOP)
        .stack_tooltip_custom(Card::new(
          Flex::column()
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            .with_child(
              Label::new("Bypasses the official launcher and").with_text_color(OLD_TEXT_COLOR),
            )
            .with_child(Label::new("starts the game immediately.").with_text_color(OLD_TEXT_COLOR))
            .padding((4.0, 0.0)),
        ))
        .with_background_color(druid::Color::TRANSPARENT)
        .with_border_color(druid::Color::TRANSPARENT),
    )
}

fn launch_button_body() -> impl Widget<App> {
  Flex::column()
    .with_child(
      Flex::row()
        .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
        .with_child(
          Icon::new(*PLAY_ARROW)
            .fix_size(50.0, 50.0)
            .padding((-10.0, 0.0, 10.0, 0.0)),
        )
        .with_child(
          Flex::column()
            .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
            .with_child(text_maker("Launch"))
            .with_child(text_maker("Starsector"))
            .padding((-10.0, 0.0, 10.0, 0.0)),
        ),
    )
    .with_default_spacer()
    .on_click(|ctx, app: &mut App, _| {
      if let Some(install_dir) = app.settings.install_dir.clone() {
        ctx.submit_command(Popup::OPEN_POPUP.with(Popup::custom(|| {
          CardButton::button_text("Running Starsector...")
            .halign_centre()
            .boxed()
        })));

        let experimental_launch = app.settings.experimental_launch;
        let experimental_resolution = app.settings.experimental_resolution;
        let ext_ctx = ctx.get_external_handle();
        app.runtime.spawn(async move {
          let install_dir = install_dir;
          launch_starsector(
            install_dir,
            experimental_launch,
            experimental_resolution,
            ext_ctx,
          )
          .await
        });
      }
    })
}

pub(crate) async fn launch_starsector(
  install_dir: PathBuf,
  experimental_launch: bool,
  resolution: (u32, u32),
  ext_ctx: ExtEventSink,
) -> anyhow::Result<()> {
  let res = launch(&install_dir, experimental_launch, resolution)
    .and_then(|child| child.wait_with_output().map_err(Into::into))
    .await;

  let matches: std::sync::Arc<dyn Fn(&Popup) -> bool + Send + Sync> =
    std::sync::Arc::new(|popup| matches!(popup, Popup::Custom(_)));
  let _ = ext_ctx.submit_command_global(Popup::DISMISS_MATCHING, matches);
  if let Err(err) = res {
    let _ = ext_ctx.submit_command_global(
      Popup::OPEN_POPUP,
      Popup::custom(move || LaunchResult::view(err.to_string()).boxed()),
    );
  }
  let _ = ext_ctx.submit_command_global(App::ENABLE, ());

  Ok(())
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) async fn launch(
  install_dir: &PathBuf,
  experimental_launch: bool,
  resolution: (u32, u32),
) -> anyhow::Result<tokio::process::Child> {
  use tokio::{fs::read_to_string, process::Command};

  Ok(if experimental_launch {
    #[cfg(target_os = "windows")]
    let vmparams_path = install_dir.join("vmparams");
    #[cfg(target_os = "linux")]
    let vmparams_path = install_dir.join("starsector.sh");

    let args_raw = read_to_string(vmparams_path).await?;
    let args: Vec<&str> = args_raw.split_ascii_whitespace().skip(1).collect();

    #[cfg(target_os = "windows")]
    let executable = install_dir.join("jre/bin/java.exe");
    #[cfg(target_os = "linux")]
    let executable = install_dir.join("jre_linux/bin/java");

    #[cfg(target_os = "windows")]
    let current_dir = install_dir.join("starsector-core");
    #[cfg(target_os = "linux")]
    let current_dir = install_dir.clone();

    Command::new(executable)
      .current_dir(current_dir)
      .args([
        "-DlaunchDirect=true",
        &format!("-DstartRes={}x{}", resolution.0, resolution.1),
        "-DstartFS=false",
        "-DstartSound=true",
      ])
      .args(args)
      .spawn()?
  } else {
    #[cfg(target_os = "windows")]
    let executable = install_dir.join("starsector.exe");
    #[cfg(target_os = "linux")]
    let executable = install_dir.join("starsector.sh");

    Command::new(executable).current_dir(install_dir).spawn()?
  })
}

#[cfg(target_os = "macos")]
pub(crate) async fn launch(
  install_dir: &std::path::Path,
  experimental_launch: bool,
  resolution: (u32, u32),
) -> anyhow::Result<tokio::process::Child> {
  use anyhow::Context;
  use tokio::process::Command;

  Ok(if experimental_launch {
    Command::new(install_dir.join("Contents/MacOS/starsector_macos.sh"))
      .current_dir(install_dir.join("Contents/MacOS"))
      .env(
        "EXTRAARGS",
        format!(
          "-DlaunchDirect=true -DstartRes={}x{} -DstartFS=false -DstartSound=true",
          resolution.0, resolution.1
        ),
      )
      .spawn()?
  } else {
    let executable = install_dir.parent().context("Get install_dir parent")?;
    let current_dir = executable.parent().context("Get install_dir parent")?;

    Command::new(executable).current_dir(current_dir).spawn()?
  })
}

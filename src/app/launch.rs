use std::{path::Path, process::Output};

use druid::{
  widget::{Container, Either, Flex, Label, TextBox, ViewSwitcher},
  Color, Data, Key, LensExt, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::{material_icons::Icon, WidgetExt as _};
use tokio::process::Command;
use webview_shared::ExtEventSinkExt;

use super::{
  overlays::{LaunchResult, Popup},
  theme::{BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
  util::{h2_fixed, LabelExt, Tap, ValueFormatter},
  App, SETTINGS, TOGGLE_ON,
};
use crate::{
  app::{
    controllers::Rotated,
    settings::Settings,
    util::{bold_text, ShadeColor, WidgetExtEx},
    CHEVRON_LEFT, CHEVRON_RIGHT, INFO, PLAY_ARROW,
  },
  patch::{
    separator::Separator,
    table::{FixedFlexTable, TableColumnWidth, TableRow},
  },
  widgets::{
    card::Card,
    card_button::{CardButton, ScopedStackCardButton},
    root_stack::RootStack,
  },
};

const OLD_TEXT_COLOR: druid::Key<druid::Color> = druid::Key::new("old_text_colour");
const OVERRIDE_HOVER: Selector<bool> = Selector::new("launch.dropdown.collapsed.hovered.supp");
const OPEN_STACK: Selector = Selector::new("launch_button.stack.open");

const ROW_HEIGHT: f64 = 64.0;

const RESOLUTIONS: &'static [(u32, u32)] = &[
  (3840, 2160),
  (3440, 1440),
  (2560, 1600),
  (2560, 1440),
  (1920, 1200),
  (1920, 1080),
  (1600, 900),
  (1440, 900),
  (1366, 768),
  (1280, 720),
  (800, 600),
  (640, 480),
];

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
    .with_shadow_length(8.0)
    .with_shadow_increase(4.0)
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
    .mask_default()
    .dynamic(|data: &App, _| {
      !data
        .settings
        .install_dir
        .as_deref()
        .is_some_and(Path::exists)
    })
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
          experimental_launch_row(true),
        ))
        .with_flex_spacer(1.0)
        .with_child(Icon::new(*CHEVRON_LEFT).fix_size(20.0, 20.0))
        .with_spacer(8.0),
    )
    .with_default_spacer()
    .background(BACKGROUND)
    .padding((1.0, 0.0, 1.0, 0.0));

  Container::new(child)
    .rounded(4.0)
    .padding((0.0, 0.0, 0.0, -6.0))
    .scope_with_hover_state(false, false, |widget| {
      widget
        .env_scope(move |env, state| {
          if state.1 {
            env.set(BACKGROUND, Color::GRAY.lighter_by(4));
            env.set(druid::theme::TEXT_COLOR, Color::BLACK);
          } else {
            env.set(BACKGROUND, Color::BLACK.interpolate_with(Color::GRAY, 1))
          };
        })
        .on_command(OVERRIDE_HOVER, |ctx, payload, data| {
          data.1 = *payload;
          ctx.request_update();
          ctx.request_paint();
        })
        .on_event(|_, ctx, _, data| {
          if ctx.is_hot() || data.1 {
            ctx.clear_cursor();
            ctx.set_cursor(&druid::Cursor::Arrow);
            ctx.override_cursor(&druid::Cursor::Arrow);
          }

          false
        })
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
          experimental_launch_row(true),
        ))
        .with_flex_spacer(1.0)
        .with_child(Rotated::new(
          Icon::new(*CHEVRON_RIGHT).fix_size(20.0, 20.0),
          1,
        ))
        .with_spacer(8.0),
    )
    .with_default_spacer()
    .with_child(
      Flex::row()
        .with_default_spacer()
        .with_child(Label::new("Official Launcher").else_if(
          |data: &App, _| !data.settings.experimental_launch,
          experimental_launch_row(false),
        ))
        .with_flex_spacer(1.0)
        .padding((0.0, 10.0))
        .background(BACKGROUND)
        .rounded(6.0)
        .scope_with_hover_state(false, false, |scoped| {
          scoped
            .env_scope(|env, data| {
              env.set(
                BACKGROUND,
                if data.1 {
                  Color::GRAY.lighter_by(8)
                } else {
                  Color::GRAY.lighter_by(4)
                },
              )
            })
            .on_event(|_, ctx, _, data| {
              if ctx.is_hot() || data.1 {
                ctx.clear_cursor();
                ctx.set_cursor(&druid::Cursor::Arrow);
                ctx.override_cursor(&druid::Cursor::Arrow);
              }

              false
            })
        })
        .on_click(|_, data, _| {
          data.settings.experimental_launch = !data.settings.experimental_launch;
          data.settings.save_async(&data.runtime)
        }),
    )
    .background(Color::GRAY.lighter_by(4))
    .padding((1.0, 0.0, 1.0, 0.0));

  Container::new(child)
    .rounded(4.0)
    .padding((0.0, 0.0, 0.0, -6.0))
    .env_scope(move |env, _| {
      env.set(druid::theme::TEXT_COLOR, Color::BLACK);
    })
}

fn experimental_launch_row(active: bool) -> Flex<App> {
  Flex::row()
    .with_child(Label::new("Skip Launcher"))
    .with_spacer(2.5)
    .with_child(
      Icon::new(*INFO)
        .fix_size(15.0, 15.0)
        .align_vertical(druid::UnitPoint::TOP)
        .stack_tooltip_custom(
          Card::builder()
            .with_background(Color::GRAY.lighter_by(7))
            .build(
              Flex::column()
                .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
                .with_child(
                  Label::new("Bypasses the official launcher.").with_text_color(Color::BLACK),
                )
                .with_child(
                  Label::wrapped(
                    "You can't change your mod list or launcher only graphics settings like \
                     resolution when this is set.",
                  )
                  .with_text_color(Color::BLACK),
                )
                .fix_width(200.0)
                .padding((4.0, 0.0)),
            ),
        )
        .with_offset((10.0, 10.0)),
    )
    .pipe(|row| {
      if active {
        row
          .with_spacer(5.0)
          .with_child(
            Icon::new(*SETTINGS)
              .fix_size(20.0, 20.0)
              .on_click(|ctx, _, _| {
                RootStack::dismiss(ctx);
                ctx.submit_command(
                  Popup::DELAYED_POPUP.with(vec![Popup::app_custom(move || resolution_options())]),
                );
              }),
          )
      } else {
        row
      }
    })
}

fn resolution_options() -> Box<dyn Widget<App>> {
  FixedFlexTable::new()
    .default_column_width(TableColumnWidth::Intrinsic)
    .with_row(
      TableRow::new().with_child(
        Card::new(
          Flex::row()
            .with_child(
              Rotated::new(Icon::new(*TOGGLE_ON), 1)
                .else_if(
                  |data: &(App, bool), _| data.1,
                  Rotated::new(Icon::new(*TOGGLE_ON), 3),
                )
                .fix_size(35.0, 35.0)
                .on_click(|_, data, _| data.1 = !data.1)
                .padding((0.0, 0.0, -4.0, 0.0))
                .wrap_with_hover_state(true, true),
            )
            .with_flex_child(
              Either::new(
                |data, _| data.1,
                preset_resolution().lens(druid::lens!((App, bool), 0)),
                custom_resolution().lens(druid::lens!((App, bool), 0)),
              ),
              1.0,
            ),
        )
        .fix_size(375.0, ROW_HEIGHT),
      ),
    )
    .with_row(
      TableRow::new().with_child(
        Flex::row()
          .with_child(
            Flex::row()
              .with_child(
                CardButton::button_with(
                  |_| CardButton::button_text("Close").padding((8.0, 0.0)),
                  Card::builder()
                    .with_background(RED_KEY)
                    .with_border(1.0, ON_RED_KEY),
                )
                .on_click(|ctx, _, _| {
                  ctx.submit_command(Popup::dismiss_matching(|pop| {
                    matches!(pop, Popup::AppCustom(_))
                  }))
                })
                .env_scope(|env, _| env.set(druid::theme::TEXT_COLOR, env.get(ON_RED_KEY))),
              )
              .with_child(
                CardButton::button_with(
                  |_| CardButton::button_text("Launch").padding((8.0, 0.0)),
                  Card::builder()
                    .with_background(BLUE_KEY)
                    .with_border(1.0, ON_BLUE_KEY),
                )
                .on_click(|ctx, data, _| {
                  ctx.submit_command(Popup::dismiss_matching(|pop| {
                    matches!(pop, Popup::AppCustom(_))
                  }));
                  managed_starsector_launch(data, ctx);
                })
                .env_scope(|env, _| env.set(druid::theme::TEXT_COLOR, env.get(ON_BLUE_KEY))),
              )
              .lens(druid::lens!((App, bool), 0)),
          )
          .align_right(),
      ),
    )
    .align_horizontal(druid::UnitPoint::CENTER)
    .on_added(|_, ctx, _, _| RootStack::dismiss(ctx))
    .lens_scope(|data| (data.clone(), true), druid::lens!((App, bool), 0))
    .boxed()
}

fn preset_resolution() -> impl Widget<App> {
  Flex::row()
    .must_fill_main_axis(true)
    .cross_axis_alignment(druid::widget::CrossAxisAlignment::Center)
    .with_child(h2_fixed("Select resolution:").padding((0.0, 0.0, 0.0, 4.0)))
    .with_flex_spacer(1.0)
    .with_child(
      Card::builder()
        .with_insets(0.1)
        .with_corner_radius(0.0)
        .with_shadow_length(0.0)
        .with_shadow_increase(0.0)
        .with_border(1.0, Color::BLACK)
        .stacked_button(
          {
            move |_| {
              Flex::row()
                .with_child(ViewSwitcher::new(
                  |data: &Option<(u32, u32)>, _| data.clone(),
                  |_, current_res, _| {
                    let res;
                    CardButton::button_text(if let Some((x, y)) = current_res {
                      res = format!("{x} x {y}");
                      &res
                    } else {
                      "None selected"
                    })
                    .boxed()
                  },
                ))
                .with_flex_spacer(1.0)
                .with_child(Icon::new(*CHEVRON_LEFT))
                .padding(8.0)
                .lens(App::settings.then(Settings::experimental_resolution))
            }
          },
          move |_| {
            Flex::column()
              .with_child(
                Flex::row()
                  .with_child(ViewSwitcher::new(
                    |data: &Option<(u32, u32)>, _| data.clone(),
                    |_, current_res, _| {
                      let res;
                      CardButton::button_text(if let Some((x, y)) = current_res {
                        res = format!("{x} x {y}");
                        &res
                      } else {
                        "None selected"
                      })
                      .boxed()
                    },
                  ))
                  .with_flex_spacer(1.0)
                  .with_child(Rotated::new(Icon::new(*CHEVRON_RIGHT), 1))
                  .padding((8.0, 6.0, 8.0, 3.0)),
              )
              .with_spacer(5.0)
              .tap(|col| {
                for res in RESOLUTIONS.iter() {
                  let (x, y) = res;
                  col.add_child(
                    Label::new(format!("{x} x {y}"))
                      .padding((3.5, 5.0))
                      .expand_width()
                      .scope_with_hover_state(false, true, |widget| {
                        const RES_BORDER_COLOR: Key<Color> =
                          Key::new("resolution_select.resolution.border.colour");
                        widget.border(RES_BORDER_COLOR, 1.0).env_scope(|env, data| {
                          env.set(
                            RES_BORDER_COLOR,
                            if data.1 {
                              Color::BLACK
                            } else {
                              Color::TRANSPARENT
                            },
                          )
                        })
                      })
                      .on_click(move |_, data: &mut Option<(u32, u32)>, _| {
                        data.replace(*res);
                      }),
                  )
                }
              })
              .on_click(|ctx, _, _| RootStack::dismiss(ctx))
              .lens(Settings::experimental_resolution)
              .on_change(|_, _, data, _| {
                let _ = data.save();
              })
              .lens(App::settings)
          },
          CardButton::stack_none(),
          130.0,
        ),
    )
    .padding((7.0, 0.0))
}

fn custom_resolution() -> impl Widget<App> {
  Flex::row()
    .must_fill_main_axis(true)
    .cross_axis_alignment(druid::widget::CrossAxisAlignment::Center)
    .with_child(h2_fixed("Custom:").padding((0.0, 0.0, 0.0, 4.0)))
    .with_flex_spacer(1.0)
    .with_child(
      TextBox::new()
        .with_placeholder("Horizontal")
        .with_formatter(ValueFormatter)
        .update_data_while_editing(true)
        .lens(druid::lens!((u32, u32), 0)),
    )
    .with_child(h2_fixed("x"))
    .with_child(
      TextBox::new()
        .with_placeholder("Vertical")
        .with_formatter(ValueFormatter)
        .update_data_while_editing(true)
        .lens(druid::lens!((u32, u32), 1)),
    )
    .padding((7.0, 0.0))
    .lens(App::settings.then(Settings::experimental_resolution).map(
      |data| data.unwrap_or_default(),
      |data, res| {
        if res != (0, 0) {
          data.replace(res);
        }
      },
    ))
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
      if app.settings.experimental_launch && app.settings.experimental_resolution.is_none() {
        RootStack::dismiss(ctx);
        ctx.submit_command(
          Popup::DELAYED_POPUP.with(vec![Popup::app_custom(move || resolution_options())]),
        );
      } else {
        managed_starsector_launch(app, ctx);
      }
    })
}

fn managed_starsector_launch(app: &mut App, ctx: &mut druid::EventCtx) {
  if let Some(install_dir) = app.settings.install_dir.clone() {
    ctx.submit_command(Popup::OPEN_POPUP.with(Popup::custom(|| {
      CardButton::button_text("Running Starsector...")
        .halign_centre()
        .boxed()
    })));

    let experimental_launch = app.settings.experimental_launch;
    let experimental_resolution = app.settings.experimental_resolution;
    let miko = app.settings.jre_23;
    let ext_ctx = ctx.get_external_handle();
    app.runtime.spawn(async move {
      let res = launch(
        &install_dir,
        experimental_launch,
        experimental_resolution,
        miko,
      )
      .await;

      let _ = ext_ctx.submit_command_global(
        Popup::DISMISS_MATCHING,
        Popup::wrap_dismiss_matching(|popup| matches!(popup, Popup::Custom(_))),
      );
      if let Err(err) = res {
        let _ = ext_ctx.submit_command_global(
          Popup::OPEN_POPUP,
          Popup::custom(move || LaunchResult::view(err.to_string()).boxed()),
        );
      }
      let _ = ext_ctx.submit_command_global(App::ENABLE, ());
    });
  }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub(crate) async fn launch(
  install_dir: &Path,
  experimental_launch: bool,
  resolution: Option<(u32, u32)>,
  miko: bool,
) -> anyhow::Result<std::process::Output> {
  use tokio::fs::read_to_string;

  Ok(if experimental_launch {
    let resolution = resolution.unwrap();

    let vmparams_path = install_dir.join(match miko {
      true => "Miko_R3.txt",
      #[cfg(target_os = "windows")]
      false => "vmparams",
      #[cfg(target_os = "linux")]
      false => "starsector.sh",
    });

    let executable = install_dir.join(match miko {
      #[cfg(target_os = "windows")]
      true => "jdk-23+7/bin/java.exe",
      #[cfg(target_os = "windows")]
      false => "jre/bin/java.exe",
      #[cfg(target_os = "linux")]
      true => "jdk-23+9/bin/java",
      #[cfg(target_os = "linux")]
      false => "jre_linux/bin/java",
    });

    #[cfg(target_os = "windows")]
    let current_dir = install_dir.join("starsector-core");
    #[cfg(target_os = "linux")]
    let current_dir = install_dir.clone();

    #[cfg(target_os = "windows")]
    if let output @ Ok(_) = elevated_windows_launch(
      &current_dir,
      &executable,
      Some((
        &vmparams_path,
        format!(
          "-DlaunchDirect=true -DstartRes={}x{} -DstartFS=false -DstartSound=true",
          resolution.0, resolution.1
        ),
      )),
    )
    .await
    {
      return output;
    }

    let args = read_to_string(&vmparams_path).await?.replacen(
      r#"-Djava.library.path="..\\mikohime/windows""#,
      r"-Djava.library.path=..\\mikohime/windows",
      1,
    );

    Command::new(executable)
      .current_dir(current_dir)
      .args(&[
        "-DlaunchDirect=true",
        &format!("-DstartRes={}x{}", resolution.0, resolution.1),
        "-DstartFS=false",
        "-DstartSound=true",
      ])
      .args(
        args
          .split_ascii_whitespace()
          .skip(if !miko { 1 } else { 0 }),
      )
      .spawn()?
      .wait_with_output()
      .await?
  } else {
    let executable = install_dir.join(match miko {
      #[cfg(target_os = "windows")]
      true => "Miko_Rouge.bat",
      #[cfg(target_os = "windows")]
      false => "starsector.exe",
      #[cfg(target_os = "linux")]
      true => "Kitsunebi.sh",
      #[cfg(target_os = "linux")]
      false => "starsector.sh",
    });

    #[cfg(target_os = "windows")]
    if let output @ Ok(_) = elevated_windows_launch(&install_dir, &executable, None).await {
      return output;
    }

    Command::new(executable)
      .current_dir(install_dir)
      .spawn()?
      .wait_with_output()
      .await?
  })
}

#[cfg(target_os = "windows")]
async fn elevated_windows_launch(
  current_dir: &Path,
  executable: &Path,
  vmparams: Option<(&Path, String)>,
) -> Result<Output, anyhow::Error> {
  use std::{ffi::*, iter::FromIterator};

  use itertools::Itertools;
  use tokio::io::AsyncWriteExt;

  use crate::d_println;

  let (vmparams_path, additional_params) = vmparams.unzip();

  let mut pwsh = Command::new("powershell.exe");
  pwsh.stdin(std::process::Stdio::piped());

  let mut process = pwsh.spawn()?;
  let mut stdin = process
    .stdin
    .take()
    .ok_or(anyhow::anyhow!("Failed to retrieve powershell input"))?;

  let base = indoc::formatdoc! {r#"
      {}
      try {{
        Start-Process -Wait -Verb RunAs -WorkingDirectory % -FilePath % {}{} | Write-Null
      }} catch {{
        if ($_.Exception.GetType().Name -eq "InvalidOperationException")
        {{
          throw "Failed to elevate"
        }}
        throw $_
      }}
    "#,
    if vmparams_path.is_some() { "$miko = Get-Content -Path %" } else { "" },
    if vmparams_path.is_some() { "-ArgumentList $miko " } else { "" },
    additional_params.unwrap_or_default()
  };

  let dummy = OsStr::new("");
  let paths = [current_dir.as_os_str(), executable.as_os_str()];
  let commands = base.split('%').map(OsStr::new).interleave_shortest(
    vmparams_path
      .map(Path::as_os_str)
      .into_iter()
      .chain(IntoIterator::into_iter(paths).chain(std::iter::repeat(dummy))),
  );
  let os_str = OsString::from_iter(commands);

  tokio::spawn(async move {
    stdin.write_all(os_str.as_encoded_bytes()).await?;

    stdin.write(OsStr::new("\n").as_encoded_bytes()).await
  })
  .await??;

  let output = process.wait_with_output().await?;

  if !output.status.success() {
    if std::str::from_utf8(&output.stderr).map_or(false, |err| err.contains("Failed to elevate")) {
      d_println!("User declined UAC prompt")
      // remove miko options that require admin?
    } else {
      d_println!("{:?}", output.stderr)
    }
    anyhow::bail!("Failed to elevate through powershell")
  }

  Ok(output)
}

#[cfg(target_os = "macos")]
pub(crate) async fn launch(
  install_dir: &Path,
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

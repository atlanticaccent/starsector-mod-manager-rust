use druid::{widget::Controller, Event, EventCtx, Target, Widget, Menu, MenuItem};
use rfd::FileDialog;

use crate::app::App;

pub struct InstallController;

impl<W: Widget<App>> Controller<App, W> for InstallController {
  fn event(
    &mut self,
    child: &mut W,
    ctx: &mut EventCtx,
    event: &Event,
    data: &mut App,
    env: &druid::Env,
  ) {
    match event {
      Event::MouseDown(mouse_event) => {
        if mouse_event.button == druid::MouseButton::Left {
          ctx.set_active(true);
          ctx.request_paint();
        }
      }
      Event::MouseUp(mouse_event) => {
        if ctx.is_active() && mouse_event.button == druid::MouseButton::Left {
          ctx.set_active(false);
          if ctx.is_hot() {
            let ext_ctx = ctx.get_external_handle();
            let menu: Menu<App> = Menu::empty()
              .entry(MenuItem::new("From Archive(s)").on_activate(
                move |_ctx, data: &mut App, _| {
                  let ext_ctx = ext_ctx.clone();
                  data.runtime.spawn_blocking(move || {
                    let res = FileDialog::new()
                      .add_filter(
                        "Archives",
                        &["zip", "7z", "7zip", "rar", "rar4", "rar5", "tar"],
                      )
                      .pick_files();

                    ext_ctx.submit_command(App::OPEN_FILE, res, Target::Auto)
                  });
                },
              ))
              .entry(MenuItem::new("From Folder").on_activate({
                let ext_ctx = ctx.get_external_handle();
                move |_ctx, data: &mut App, _| {
                  data.runtime.spawn_blocking({
                    let ext_ctx = ext_ctx.clone();
                    move || {
                      let res = FileDialog::new().pick_folder();

                      ext_ctx.submit_command(App::OPEN_FOLDER, res, Target::Auto)
                    }
                  });
                }
              }));

            ctx.show_context_menu::<App>(menu, ctx.to_window(mouse_event.pos))
          }
          ctx.request_paint();
        }
      }
      _ => {}
    }

    child.event(ctx, event, data, env);
  }
}
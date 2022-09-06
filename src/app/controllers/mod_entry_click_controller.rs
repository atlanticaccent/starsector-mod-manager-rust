use std::sync::Arc;

use druid::{widget::Controller, Event, EventCtx, Menu, MenuItem, Widget};
use tap::Pipe;

use crate::app::{
  mod_description::{ModDescription, OPEN_IN_BROWSER},
  mod_entry::ModEntry,
  App,
};

pub struct ModEntryClickController;

impl<W: Widget<Arc<ModEntry>>> Controller<Arc<ModEntry>, W> for ModEntryClickController {
  fn event(
    &mut self,
    child: &mut W,
    ctx: &mut EventCtx,
    event: &Event,
    data: &mut Arc<ModEntry>,
    env: &druid::Env,
  ) {
    match event {
      Event::MouseDown(mouse_event) => {
        if mouse_event.button == druid::MouseButton::Right {
          ctx.set_active(true);
          ctx.request_paint();
        }
      }
      Event::MouseUp(mouse_event) => {
        if ctx.is_active() && mouse_event.button == druid::MouseButton::Right {
          ctx.set_active(false);
          if ctx.is_hot() {
            let menu = Menu::empty()
              .entry(MenuItem::new("Open in File Browser").on_activate({
                let entry = data.clone();
                move |_, _, _| {
                  if let Err(err) = opener::open(entry.path.clone()) {
                    eprintln!("{}", err)
                  }
                }
              }))
              .pipe(|mut menu| {
                if let Some(fractal_id) =
                  data.version_checker.as_ref().map(|v| v.fractal_id.clone())
                {
                  if !fractal_id.is_empty() {
                    menu = menu.entry(
                      MenuItem::new("Open post on Fractalsoftworks Forum").on_activate(
                        move |ctx, _, _| {
                          ctx.submit_command(OPEN_IN_BROWSER.with(format!(
                            "{}{}",
                            ModDescription::FRACTAL_URL,
                            fractal_id
                          )))
                        },
                      ),
                    )
                  }
                }
                if let Some(nexus_id) = data.version_checker.as_ref().map(|v| v.nexus_id.clone()) {
                  if !nexus_id.is_empty() {
                    menu = menu.entry(MenuItem::new("Open post on Nexusmods").on_activate(
                      move |ctx, _, _| {
                        ctx.submit_command(OPEN_IN_BROWSER.with(format!(
                          "{}{}",
                          ModDescription::NEXUS_URL,
                          nexus_id
                        )))
                      },
                    ))
                  }
                }

                menu
              })
              .entry(MenuItem::new("Delete").on_activate({
                let entry = data.clone();
                move |ctx, _, _| ctx.submit_command(ModEntry::ASK_DELETE_MOD.with(entry.clone()))
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

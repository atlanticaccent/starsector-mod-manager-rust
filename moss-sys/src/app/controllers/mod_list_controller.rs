use druid::{widget::Controller, Env, Event, EventCtx, Widget};

use crate::app::{installer, mod_list::ModList, App};

pub struct ModListController;

impl<W: Widget<App>> Controller<App, W> for ModListController {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut App, env: &Env) {
    if let Event::Command(cmd) = event {
      if let Some((conflict, install_from, entry)) = cmd.get(ModList::OVERWRITE) {
        if let Some(install_dir) = &data.settings.install_dir {
          ctx.submit_command(App::LOG_MESSAGE.with(format!("Resuming install for {}", entry.name)));
          data.runtime.spawn(
            installer::Payload::Resumed(
              Box::new(entry.clone()),
              install_from.clone(),
              conflict.clone(),
            )
            .install(
              ctx.get_external_handle(),
              install_dir.clone(),
              data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
            ),
          );
        }
        ctx.is_handled();
      }
    }

    child.event(ctx, event, data, env);
  }
}

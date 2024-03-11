use druid::{
  lens,
  widget::{Controller, Label, Maybe},
  Env, Event, EventCtx, Widget, WidgetExt,
};

use crate::app::{
  installer::{self, ChannelMessage},
  mod_entry::{ModEntry, UpdateStatus},
  mod_list::ModList,
  modal::Modal,
  util::{get_master_version, LabelExt},
  App,
};

pub struct ModListController;

impl<W: Widget<App>> Controller<App, W> for ModListController {
  fn event(&mut self, child: &mut W, ctx: &mut EventCtx, event: &Event, data: &mut App, env: &Env) {
    if let Event::Command(cmd) = event {
      if let Some((conflict, install_to, entry)) = cmd.get(ModList::OVERWRITE) {
        if let Some(install_dir) = &data.settings.install_dir {
          ctx.submit_command(App::LOG_MESSAGE.with(format!("Resuming install for {}", entry.name)));
          data.runtime.spawn(
            installer::Payload::Resumed(entry.clone(), install_to.clone(), conflict.clone())
              .install(
                ctx.get_external_handle(),
                install_dir.clone(),
                data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
              ),
          );
        }
        ctx.is_handled();
      } else if let Some(payload) = cmd.get(installer::INSTALL) {
        match payload {
          ChannelMessage::Success(entry) => {
            let mut entry = entry.clone();
            if let Some(existing) = data.mod_list.mods.get(&entry.id) {
              entry.enabled = existing.enabled;
              if let Some(remote_version_checker) = existing.remote_version.clone() {
                entry.remote_version = Some(remote_version_checker.clone());
                entry.update_status = Some(UpdateStatus::from((
                  entry.version_checker.as_ref().unwrap(),
                  &Some(remote_version_checker),
                )));
              }
            }
            ctx.submit_command(App::LOG_SUCCESS.with(entry.name.clone()));
            data.mod_list.mods.insert(entry.id.clone(), entry.into());
            data.mod_list.filter_state.sorted_ids = data.mod_list.sorted_vals().cloned().collect();
            ctx.request_update();
          }
          ChannelMessage::Duplicate(conflict, to_install, entry) => ctx.submit_command(
            App::LOG_OVERWRITE.with((conflict.clone(), to_install.clone(), entry.clone())),
          ),
          ChannelMessage::FoundMultiple(source, found_paths) => {
            ctx.submit_command(App::FOUND_MULTIPLE.with((source.clone(), found_paths.clone())));
          }
          ChannelMessage::Error(name, err) => {
            ctx.submit_command(App::LOG_ERROR.with((name.clone(), err.clone())));
            eprintln!("Failed to install {}", err);
          }
        }
      }
    } else if let Event::Notification(notif) = event {
      if let Some(entry) = notif.get(ModEntry::AUTO_UPDATE) {
        Modal::<()>::new("Auto-update?")
          .with_content(format!("Would you like to automatically update {}?", entry.name))
          .with_content(format!("Installed version: {}", entry.version))
          .with_content(format!(
            "New version: {}",
            entry
              .remote_version
              .as_ref()
              .map(|v| v.version.to_string())
              .unwrap_or_else(|| String::from(
                "Error: failed to retrieve version, this shouldn't be possible."
              ))
          ))
          .with_content(
            Maybe::or_empty(|| Label::wrapped("\
              NOTE: A .git directory has been detected in the target directory. \
              Are you sure this isn't being used for development?\
            "))
            .lens(
              lens::Constant(data.settings.git_warn.then(|| {
                if entry.path.join(".git").exists() {
                  Some(())
                } else {
                  None
                }
              }).flatten())
            )
            .boxed()
          )
          .with_content("WARNING:")
          .with_content("Save compatibility is not guaranteed when updating a mod. Your save may no longer load if you apply this update.")
          .with_content("Bug reports about saves broken by using this feature will be ignored.")
          .with_content("YOU HAVE BEEN WARNED")
          .with_button("Update", ModList::AUTO_UPDATE.with(entry.into()))
          .with_close_label("Cancel");
        // .show_with_size(ctx, env, &(), (600., 300.));
      }
    }

    child.event(ctx, event, data, env)
  }
}

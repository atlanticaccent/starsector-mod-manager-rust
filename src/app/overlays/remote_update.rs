use std::sync::Arc;

use druid::{
  widget::{Either, Flex, Label},
  Data, Key, Widget, WidgetExt,
};
use druid_widget_nursery::material_icons::Icon;

use super::Popup;
use crate::{
  app::{
    installer,
    mod_entry::{ModVersionMeta, Version},
    util::{
      h2_fixed, DataTimer, WidgetExtEx as _, WithHoverState, BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY,
      RED_KEY,
    },
    App, CONTENT_COPY, DONE_ALL,
  },
  patch::table::{FixedFlexTable, TableColumnWidth, TableRow},
  widgets::card::Card,
};

#[derive(Clone, Data)]
pub struct RemoteUpdate {
  mod_id: String,
  mod_name: String,
  local_version: Version,
  remote_version: ModVersionMeta,
}

impl RemoteUpdate {
  pub fn new(
    mod_id: String,
    mod_name: String,
    local_version: Version,
    remote_version: ModVersionMeta,
  ) -> Self {
    Self {
      mod_id,
      mod_name,
      local_version,
      remote_version,
    }
  }

  pub fn view(&self) -> impl Widget<App> {
    let Self {
      mod_id,
      mod_name,
      local_version,
      remote_version,
    } = self;
    let mod_id = mod_id.clone();
    let remote_version = remote_version.clone();
    let direct_download_url = remote_version.direct_download_url.clone().unwrap();

    Card::builder()
      .with_insets(Card::CARD_INSET)
      .with_background(druid::theme::BACKGROUND_LIGHT)
      .build(
        Flex::column()
          .with_child(h2_fixed(&format!(
            r#"Would you like to update {mod_name} to the latest version?"#
          )))
          .with_child(Label::new("The new version will be downloaded from:"))
          .with_child(
            Flex::row()
              .with_child(Label::new(direct_download_url.clone()))
              .with_child(
                Either::new(
                  |data: &DataTimer, _| data.same(&DataTimer::INVALID),
                  Icon::new(*CONTENT_COPY),
                  Icon::new(*DONE_ALL),
                )
                .fix_width(24.)
                .on_click(move |ctx, data, _| {
                  let mut clipboard = druid::Application::global().clipboard();
                  clipboard.put_string(direct_download_url.clone());
                  if data.same(&DataTimer::INVALID) {
                    *data = ctx.request_timer(std::time::Duration::from_secs(2)).into();
                  }
                })
                .on_event(|_, _, event, data| {
                  if let druid::Event::Timer(token) = event {
                    if data == token {
                      *data = DataTimer::INVALID
                    }
                  }
                  false
                })
                .scope_independent(|| DataTimer::INVALID)
                .with_hover_state(false),
              ),
          )
          .with_default_spacer()
          .with_child(
            FixedFlexTable::new()
              .default_column_width(TableColumnWidth::Intrinsic)
              .with_row(
                TableRow::new()
                  .with_child(Label::new("The current version is:").align_left())
                  .with_child(Label::new(local_version.to_string()).align_left()),
              )
              .with_row(
                TableRow::new()
                  .with_child(Label::new("The new version will be:").align_left())
                  .with_child(Label::new(remote_version.version.to_string()).align_left()),
              ),
          )
          .with_default_spacer()
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
                  .on_click(move |ctx, data: &mut App, _| {
                    ctx.submit_command(Popup::DISMISS);
                    let old = data.mod_list.mods.get_mut(&mod_id).unwrap();
                    let new = Arc::make_mut(old);
                    new.view_state.updating = true;
                    let new = Arc::new(new.clone());
                    data.mod_list.mods[&mod_id] = new;
                    data.runtime.spawn(
                      installer::Payload::Download {
                        mod_id: mod_id.clone(),
                        old_path: data.mod_list.mods[&mod_id].path.clone(),
                        remote_version: remote_version.clone(),
                      }
                      .install(
                        ctx.get_external_handle(),
                        data.settings.install_dir.clone().unwrap(),
                        data.mod_list.mods.values().map(|v| v.id.clone()).collect(),
                      ),
                    );
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
                  .on_click(|ctx, _, _| ctx.submit_command(Popup::DISMISS)),
              ),
          ),
      )
  }
}

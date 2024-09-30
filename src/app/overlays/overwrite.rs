use druid::{
  widget::{Flex, Label},
  Data, Key, Widget, WidgetExt,
};

use super::Popup;
use crate::{
  app::{
    installer::{HybridPath, StringOrPath},
    mod_entry::ModEntry,
    theme::{BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
    util::{h2_fixed, WidgetExtEx as _},
    App,
  },
  widgets::card::Card,
};

#[derive(Clone, Data)]
pub struct Overwrite {
  #[data(ignore)]
  conflict: StringOrPath,
  #[data(ignore)]
  to_install: HybridPath,
  entry: ModEntry,
}

impl Overwrite {
  pub fn new(conflict: StringOrPath, to_install: HybridPath, entry: ModEntry) -> Self {
    Self {
      conflict,
      to_install,
      entry,
    }
  }

  pub fn view(&self) -> impl Widget<App> {
    let Self {
      conflict,
      to_install,
      entry,
    } = self.clone();

    Card::builder()
      .with_insets(Card::CARD_INSET)
      .with_background(druid::theme::BACKGROUND_LIGHT)
      .build(
        Flex::column()
          .with_child(h2_fixed(&format!(
            r#"Are you sure you want to ovewrite "{}"?"#,
            entry.name
          )))
          .with_child(Label::new(match &conflict {
            StringOrPath::String(id) => {
              format!("A mod with ID {id} alread exists.")
            }
            StringOrPath::Path(path) => format!(
              "Found a folder at the path {} when trying to install {}.",
              path.to_string_lossy(),
              entry.id
            ),
          }))
          .with_child(Label::new("This action is permanent and cannot be undone."))
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
                      .with_child(Label::new("Overwrite").padding((10.0, 0.0)))
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
                    ctx.submit_command(crate::app::mod_list::ModList::OVERWRITE.with((
                      match &conflict {
                        StringOrPath::String(id) => {
                          data.mod_list.mods.get(id).unwrap().path.clone()
                        }
                        StringOrPath::Path(path) => path.clone(),
                      },
                      to_install.clone(),
                      entry.clone(),
                    )));
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

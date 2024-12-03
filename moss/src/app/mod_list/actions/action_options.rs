use std::rc::Rc;

use common::{labels::bold_text, widget_ext::WidgetExtEx as _, widgets::card::Card};
use druid::{theme, Widget, WidgetExt};
use druid_widget_nursery::WidgetExt as _;

use super::action_button::ActionsButton;
use crate::app::mod_list::{install::install_options::InstallOptions, ModList};

pub struct ActionsOptions;

impl ActionsOptions {
  pub fn view() -> impl Widget<ModList> {
    let text = |text| {
      bold_text(
        text,
        druid::theme::TEXT_SIZE_NORMAL,
        druid::FontWeight::SEMI_BOLD,
        druid::theme::TEXT_COLOR,
      )
      .padding((8.0, 0.0))
    };

    let mut width_linker = None;
    Card::builder()
      .with_insets((0.0, 14.0))
      .with_corner_radius(4.0)
      .with_shadow_length(8.0)
      .with_background(theme::BACKGROUND_DARK)
      .build(
        ActionsButton::button_styling(
          ActionsButton::inner(true)
            .with_spacer(4.0)
            .with_child(
              Card::hoverable(
                || text("Enable All").center().fix_height(24.0).expand_width(),
                (0.0, 10.0),
              )
              .link_height_with(&mut width_linker)
              .horizontal()
              .on_click(|_, data: &mut ModList, _| {
                data.actions_state.open = false;
                for (_, entry) in data.mods.iter_mut() {
                  let entry = Rc::make_mut(entry);
                  
                  entry.enabled = true;
                }
              }),
            )
            .with_child(
              Card::hoverable(
                || text("Disable All").center().fix_height(24.0).expand_width(),
                (0.0, 10.0),
              )
              .link_height_with(&mut width_linker)
              .on_click(|_, data, _| {
                data.actions_state.open = false;
                for (_, entry) in data.mods.iter_mut() {
                  let entry = Rc::make_mut(entry);
                  entry.enabled = false;
                }
              }),
            )
            .in_layout_repeater(),
        )
        .fix_height(128.0)
        .padding((-8.0, 0.0, -8.0, -4.0)),
      )
      .empty_if_not(|data: &ModList, _| data.actions_state.open)
      .on_command(InstallOptions::DISMISS, |ctx, payload, data| {
        let hitbox = ctx
          .size()
          .to_rect()
          .with_origin(ctx.to_window((0.0, 0.0).into()));
        data.actions_state.open = hitbox.contains(*payload);
      })
      .fix_width(super::INSTALL_WIDTH)
  }
}

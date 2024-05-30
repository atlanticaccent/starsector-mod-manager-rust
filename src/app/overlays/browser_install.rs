use druid::{
  widget::{Flex, Label},
  Data, Key, Lens, Widget, WidgetExt as _,
};

use super::Popup;
use crate::{
  app::{
    browser::Browser,
    util::{h2_fixed, LabelExt, WidgetExtEx as _, BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
    App,
  },
  widgets::card::Card,
};

#[derive(Debug, Clone, Data, Lens)]
pub struct BrowserInstall {
  url: String,
}

impl BrowserInstall {
  pub fn new(url: String) -> Self {
    Self { url }
  }

  pub fn view(&self) -> impl Widget<App> {
    let Self { url } = self;
    Card::builder()
      .with_insets(Card::CARD_INSET)
      .with_background(druid::theme::BACKGROUND_LIGHT)
      .build(
        Flex::column()
          .with_child(h2_fixed("Are you trying to install a mod?"))
          .with_default_spacer()
          .with_child(Label::wrapped("Mod will be installed from this url:"))
          .with_child(Label::wrapped(url.as_str()))
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
                    if data.current_tab == crate::nav_bar::NavLabel::WebBrowser {
                      ctx.submit_command(Browser::WEBVIEW_SHOW)
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
                  .on_click(|ctx, data: &mut App, _| {
                    ctx.submit_command(Popup::DISMISS);
                    if data.current_tab == crate::nav_bar::NavLabel::WebBrowser {
                      ctx.submit_command(Browser::WEBVIEW_SHOW)
                    }
                  }),
              ),
          ),
      )
  }
}

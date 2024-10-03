use druid::{widget::Flex, Data, Widget};

use crate::app::updater::{CopyTx, Release};

#[derive(Debug, Clone, Data)]
pub enum Status {
  Ready(Release, #[cfg(not(target_os = "macos"))] CopyTx),
  Completed,
  CheckFailed(String),
  InstallFailed,
}

impl Status {
  pub fn view<T: Data>(&self) -> impl Widget<T> {
    match self {
      #[cfg(not(target_os = "macos"))]
      Status::Ready(release, tx) => prompt_update(release, tx),
      #[cfg(target_os = "macos")]
      Status::Ready(release) => prompt_update(release),
      Status::Completed => todo!(),
      Status::CheckFailed(error) => todo!(),
      Status::InstallFailed => todo!(),
    }
  }
}

#[cfg(not(target_os = "macos"))]
fn prompt_update<T: Data>(release: &Release, tx: &CopyTx) -> impl Widget<T> {
  use druid::{
    text::{FontWeight, RichTextBuilder},
    widget::{Label, RawLabel},
    Key, WidgetExt,
  };
  use self_update::cargo_crate_version;

  use crate::{
    app::{
      overlays::Popup,
      util::{h2_fixed, WidgetExtEx},
    },
    theme::{BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
    widgets::card::Card,
  };

  Flex::row()
    .must_fill_main_axis(true)
    .with_flex_spacer(0.5)
    .with_flex_child(
      Card::builder()
        .with_insets(Card::CARD_INSET)
        .with_background(druid::theme::BACKGROUND_LIGHT)
        .build(
          Flex::column()
            .with_child(h2_fixed("An update is available for MOSS."))
            .with_child({
              let mut builder = RichTextBuilder::new();
              builder
                .push("Current:")
                .weight(FontWeight::MEDIUM)
                .underline(true);
              builder.push("  ");
              builder.push(cargo_crate_version!());

              RawLabel::new().constant(builder.build())
            })
            .with_child({
              let mut builder = RichTextBuilder::new();
              builder
                .push("Newest:")
                .weight(FontWeight::MEDIUM)
                .underline(true);
              builder.push("  ");
              builder.push(&release.version);

              RawLabel::new().constant(builder.build())
            })
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
                    .on_click({
                      let tx = tx.clone();
                      move |ctx, _, _| {
                        ctx.submit_command(Popup::DISMISS);
                        tx.send(true);
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
                    .on_click({
                      let tx = tx.clone();
                      move |ctx, _, _| {
                        ctx.submit_command(Popup::DISMISS);
                        tx.send(false);
                      }
                    }),
                ),
            )
            .expand_width(),
        ),
      1.0,
    )
    .with_flex_spacer(0.5)

  //   let local_tag = TAG.strip_prefix('v').unwrap_or(TAG);
  //   let release_tag = release
  //     .tag_name
  //     .strip_prefix('v')
  //     .unwrap_or(&release.tag_name);
  //   if let Ok(true) = bump_is_greater(local_tag, release_tag) {
  //     Modal::new("Update Mod Manager?")
  //       .with_content("A new version of Starsector Mod Manager is
  // available.")       .with_content(format!("Current version: {TAG}"))
  //       .with_content(format!("New version: {}", release.tag_name))
  //       .with_content({
  //         #[cfg(not(target_os = "macos"))]
  //         let label = "Would you like to update now?";
  //         #[cfg(target_os = "macos")]
  //         let label = "Would you like to open the update in your browser?";

  //         label
  //       })
  //       .with_button("Update", App::SELF_UPDATE)
  //       .with_close_label("Cancel")
  //   } else {
  //     return;
  //   }
}

#[cfg(target_os = "macos")]
fn prompt_update<T: Data>(release: &Release) -> impl Widget<T> {
  Flex::column().with_flex_spacer(0.5).with_flex_spacer(0.5)

  // if opener::open("https://github.com/atlanticaccent/starsector-mod-manager-rust/releases").is_err()
  // {
  //   eprintln!("Failed to open GitHub");
  // }
}

// let _widget: Modal<'_, ()> = if let Ok(release) = payload {
// } else {
//   Modal::new("Error")
//     .with_content("Failed to retrieve Mod Manager update status.")
//     .with_content("There may or may not be an update available.")
//     .with_close()
// };

// let original_exe = std::env::current_exe();
// if support_self_update() && original_exe.is_ok() {
//   let widget = if self_update().is_ok() {
//     Modal::new("Restart?")
//       .with_content("Update complete.")
//       .with_content("Would you like to restart?")
//       .with_button(
//         "Restart",
//         App::RESTART
//           .with(original_exe.as_ref().unwrap().clone())
//           .to(Target::Global),
//       )
//       .with_close_label("Cancel")
//   } else {
//     Modal::new("Error")
//       .with_content("Failed to update Mod Manager.")
//       .with_content(
//         "It is recommended that you restart and check that the Manager has
// not been \           corrupted.",
//       )
//       .with_close()
//   };
//   widget.show(ctx, env, &());
// } else {
//   open_in_browser();
// }

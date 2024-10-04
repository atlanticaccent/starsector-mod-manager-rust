use std::marker::PhantomData;

use druid::{
  text::{FontWeight, RichTextBuilder},
  widget::{Flex, Label, RawLabel},
  Data, Key, Widget, WidgetExt,
};
use self_update::cargo_crate_version;

use crate::{
  app::{
    overlays::Popup,
    updater::{CopyTx, Release},
    util::{h2_fixed, hyperlink, LabelExt, WidgetExtEx},
  },
  theme::{BLUE_KEY, ON_BLUE_KEY, ON_RED_KEY, RED_KEY},
  widgets::card::Card,
};

#[derive(Debug, Clone, Data)]
pub enum Status {
  Ready(Release, CopyTx),
  Completed,
  CheckFailed(String),
  InstallFailed,
}

impl Status {
  pub fn view<T: Data>(&self) -> impl Widget<T> {
    match self {
      Status::Ready(release, tx) => View::prompt_update(release, tx).boxed(),
      Status::Completed => todo!(),
      Status::CheckFailed(error) => todo!(),
      Status::InstallFailed => todo!(),
    }
  }
}

struct View<T>(PhantomData<T>);

impl<T: Data> View<T> {
  const _ZST: () = assert!(size_of::<View<crate::app::App>>() == 0);

  fn prompt_update(release: &Release, tx: &CopyTx) -> impl Widget<T> {
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
  }

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

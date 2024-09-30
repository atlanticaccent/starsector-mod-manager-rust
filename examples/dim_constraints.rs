use druid::{
  widget::{Axis, Label},
  AppLauncher, Color, Widget, WidgetExt, WindowDesc,
};
use moss::{
  app::{
    controllers::{next_id, LayoutRepeater, SharedConstraint},
    util::LabelExt,
  },
  patch::table::{FixedFlexTable, TableRow},
};

fn main() -> Result<(), druid::PlatformError> {
  let window = WindowDesc::new(ui_builder());

  AppLauncher::with_window(window).launch(())
}

const LONG_TEXT: &str = r#"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum."

Section 1.10.32 of "de Finibus Bonorum et Malorum", written by Cicero in 45 BC

"Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium, totam rem aperiam, eaque ipsa quae ab illo inventore veritatis et quasi architecto beatae vitae dicta sunt explicabo. Nemo enim ipsam voluptatem quia voluptas sit aspernatur aut odit aut fugit, sed quia consequuntur magni dolores eos qui ratione voluptatem sequi nesciunt. Neque porro quisquam est, qui dolorem ipsum quia dolor sit amet, consectetur, adipisci velit, sed quia non numquam eius modi tempora incidunt ut labore et dolore magnam aliquam quaerat voluptatem. Ut enim ad minima veniam, quis nostrum exercitationem ullam corporis suscipit laboriosam, nisi ut aliquid ex ea commodi consequatur? Quis autem vel eum iure reprehenderit qui in ea voluptate velit esse quam nihil molestiae consequatur, vel illum qui dolorem eum fugiat quo voluptas nulla pariatur?"#;

fn ui_builder() -> impl Widget<()> {
  LayoutRepeater::new(
    next_id(),
    FixedFlexTable::new()
      .with_row(
        TableRow::new()
          .with_child(SharedConstraint::new(
            Label::wrapped("foo").background(Color::BLUE),
            0,
            Axis::Vertical,
          ))
          .with_child(SharedConstraint::new(
            Label::wrapped(LONG_TEXT).background(Color::OLIVE),
            0,
            Axis::Vertical,
          )),
      )
      .with_row(
        TableRow::new()
          .with_child(SharedConstraint::new(
            Label::wrapped("bar").background(Color::TEAL),
            0,
            Axis::Vertical,
          ))
          .with_child(SharedConstraint::new(
            Label::wrapped("baz").background(Color::GREEN),
            1,
            Axis::Vertical,
          )),
      ),
  )
}

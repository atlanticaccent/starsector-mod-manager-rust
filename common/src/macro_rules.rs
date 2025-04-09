#[macro_export]
macro_rules! replace_expr {
  ($_t:tt $sub:expr) => {
    $sub
  };
}

#[macro_export]
macro_rules! flex {
  [row, $($child:tt)*] => {
    $crate::macro_rules::row![$($child)*]};
  [column, $($child:tt)*] => {
    column![$($child;)*]
  };
}

#[macro_export]
macro_rules! row {
  [$($(.$spacer:tt)? $($child:expr)? $(, $ratio:literal)?);*] => {
    {
      let mut flex = druid::widget::Flex::row();
      $(
        $crate::macro_rules::flex_child!(flex, $($spacer)? $($child)? $(, $ratio)?);
      )*
      flex
    }
  };
}

#[macro_export]
macro_rules! column {
  [$($(.$spacer:tt)? $($child:expr)? $(, $ratio:literal)?);*] => {
    {
      let mut flex = druid::widget::Flex::column();
      $(
        $crate::macro_rules::flex_child!(flex, $($spacer)? $($child)? $(, $ratio)?);
      )*
      flex
    }
  };
}

#[macro_export]
macro_rules! flex_child {
  ($flex:ident, flex, $ratio:literal) => {
    $flex.add_flex_spacer($ratio as f64)
  };
  ($flex:ident, spacer, $size:literal) => {
    $flex.add_spacer($size as f64)
  };
  ($flex:ident, spacer) => {
    $flex.add_default_spacer( as f64)
  };
  ($flex:ident, $widget:expr, $ratio:literal) => {
    $flex.add_flex_child($widget, $ratio as f64)
  };
  ($flex:ident, $widget:expr) => {
    $flex.add_child($widget)
  };
}

pub use column;
pub use flex;
pub use replace_expr;
pub use row;
pub use flex_child;

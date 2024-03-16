use druid::{
  widget::{SizedBox, ViewSwitcher}, Data, Selector, Widget, WidgetExt
};

mod confirm_delete;
use confirm_delete::*;

use super::{mod_entry::ModEntry, App};

#[derive(Debug, Clone, PartialEq, Data)]
pub enum Popup {
  ConfirmDelete(ModEntry),
}

impl Popup {
  pub const DISMISS: Selector = Selector::new("app.popup.dismiss");
  pub const OPEN_POPUP: Selector<Popup> = Selector::new("app.popup.open");

  pub fn view() -> impl Widget<App> {
    ViewSwitcher::new(
      |data: &App, _| data.popup.clone(),
      |_, data, _| if let Some(popup) = &data.popup {
        match popup {
          Popup::ConfirmDelete(entry) => ConfirmDelete::view(entry).boxed(),
        }
      } else {
        SizedBox::empty().boxed()
      },
    )
  }
}

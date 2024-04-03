use std::rc::Rc;

use druid::{
  widget::{SizedBox, ViewSwitcher},
  Data, Selector, Widget, WidgetExt,
};

mod confirm_delete;
use confirm_delete::*;

use super::{mod_entry::ModEntry, util::WidgetExtEx};

#[derive(Clone, Data)]
pub enum Popup {
  ConfirmDelete(ModEntry),
  Custom(Rc<dyn Fn() -> Box<dyn Widget<()>>>),
}

impl Popup {
  pub const DISMISS: Selector = Selector::new("app.popup.dismiss");
  pub const OPEN_POPUP: Selector<Popup> = Selector::new("app.popup.open");

  pub fn view() -> impl Widget<Option<Popup>> {
    ViewSwitcher::new(
      |data: &Option<Popup>, _| data.clone(),
      |_, data, _| {
        if let Some(popup) = &data {
          match popup {
            Popup::ConfirmDelete(entry) => ConfirmDelete::view(entry).boxed(),
            Popup::Custom(maker) => maker().constant(()).boxed(),
          }
        } else {
          SizedBox::empty().boxed()
        }
      },
    )
  }

  pub fn custom(maker: impl Fn() -> Box<dyn Widget<()>> + 'static) -> Popup {
    Popup::Custom(Rc::new(maker))
  }
}

use std::rc::Rc;

use druid::{
  widget::{SizedBox, ViewSwitcher},
  Data, Selector, Widget, WidgetExt,
};

mod confirm_delete;
mod select_install;

use confirm_delete::*;
use select_install::*;

use super::{mod_entry::ModEntry, util::WidgetExtEx, App};

#[derive(Clone, Data)]
pub enum Popup {
  ConfirmDelete(ModEntry),
  SelectInstall,
  Custom(Rc<dyn Fn() -> Box<dyn Widget<()>>>),
}

impl Popup {
  pub const DISMISS: Selector = Selector::new("app.popup.dismiss");
  pub const OPEN_POPUP: Selector<Popup> = Selector::new("app.popup.open");

  pub fn view() -> impl Widget<App> {
    ViewSwitcher::new(
      |data: &App, _| data.popup.clone(),
      |data, _, _| {
        if let Some(popup) = &data {
          match popup {
            Popup::ConfirmDelete(entry) => ConfirmDelete::view(entry).boxed(),
            Popup::SelectInstall => SelectInstall::view().boxed(),
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

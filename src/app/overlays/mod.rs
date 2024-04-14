use std::rc::Rc;

use druid::{
  widget::{Align, SizedBox, ViewSwitcher},
  Data, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::{Mask, WidgetExt as _};

mod confirm_delete;
mod duplicate;
mod overwrite;
mod select_install;

use confirm_delete::*;
use duplicate::*;
use overwrite::*;
use select_install::*;

use super::{
  installer::{HybridPath, StringOrPath},
  mod_entry::ModEntry,
  util::WidgetExtEx,
  App,
};

#[derive(Clone, Data)]
pub enum Popup {
  ConfirmDelete(ModEntry),
  SelectInstall,
  Ovewrite(Overwrite),
  Duplicate(Duplicate),
  Custom(Rc<dyn Fn() -> Box<dyn Widget<()>>>),
}

impl Popup {
  pub const DISMISS: Selector = Selector::new("app.popup.dismiss");
  pub const OPEN_POPUP: Selector<Popup> = Selector::new("app.popup.open");
  pub const QUEUE_POPUP: Selector<Popup> = Selector::new("app.popup.queue");

  pub fn overlay(widget: impl Widget<App> + 'static) -> impl Widget<App> {
    Mask::new(widget)
      .with_mask(Align::centered(Popup::view()))
      .dynamic(|data, _| !data.popup.is_empty())
      .on_command(Popup::OPEN_POPUP, |_, popup, data| {
        data.popup.push_front(popup.clone())
      })
      .on_command(Popup::QUEUE_POPUP, |_, popup, data| {
        data.popup.push_back(popup.clone())
      })
      .on_command(Popup::DISMISS, |_, _, data| {
        data.popup.pop_front();
      })
  }

  pub fn view() -> impl Widget<App> {
    ViewSwitcher::new(
      |data: &App, _| data.popup.clone(),
      |data, _, _| {
        if let Some(popup) = &data.front() {
          match popup {
            Popup::ConfirmDelete(entry) => ConfirmDelete::view(entry).boxed(),
            Popup::SelectInstall => SelectInstall::view().boxed(),
            Popup::Ovewrite(overwrite) => Overwrite::view(overwrite).boxed(),
            Popup::Duplicate(duplicate) => Duplicate::view(duplicate).boxed(),
            Popup::Custom(maker) => maker().constant(()).boxed(),
          }
        } else {
          SizedBox::empty().boxed()
        }
      },
    )
  }

  pub fn overwrite(conflict: &StringOrPath, to_install: &HybridPath, entry: &ModEntry) -> Popup {
    Popup::Ovewrite(Overwrite::new(conflict, to_install, entry))
  }

  pub fn custom(maker: impl Fn() -> Box<dyn Widget<()>> + 'static) -> Popup {
    Popup::Custom(Rc::new(maker))
  }
}

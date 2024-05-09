use std::sync::Arc;

use druid::{
  im::Vector,
  widget::{Align, SizedBox, ViewSwitcher},
  Command, Data, Selector, Widget, WidgetExt,
};
use druid_widget_nursery::{Mask, WidgetExt as _};

mod confirm_delete;
mod duplicate;
mod multiple;
mod overwrite;
mod remote_update;
mod select_install;

use confirm_delete::*;
use duplicate::*;
use multiple::*;
use overwrite::*;
use remote_update::*;
use select_install::*;

use super::{
  controllers::MaskController,
  installer::{HybridPath, StringOrPath},
  mod_entry::ModEntry,
  util::{DataTimer, WidgetExtEx},
  App,
};

#[derive(Clone, Data)]
pub enum Popup {
  ConfirmDelete(ModEntry),
  SelectInstall,
  Ovewrite(Overwrite),
  Duplicate(Duplicate),
  FoundMultiple(Multiple),
  RemoteUpdate(RemoteUpdate),
  Custom(Arc<dyn Fn() -> Box<dyn Widget<()>> + Send + Sync>),
}

impl Popup {
  pub const DISMISS: Selector = Selector::new("app.popup.dismiss");
  pub const DISMISS_MATCHING: Selector<Arc<dyn Fn(&Popup) -> bool>> =
    Selector::new("app.popup.dismiss_matching");
  pub const OPEN_POPUP: Selector<Popup> = Selector::new("app.popup.open");
  pub const QUEUE_POPUP: Selector<Popup> = Selector::new("app.popup.queue");
  pub const DELAYED_POPUP: Selector<Vec<Popup>> = Selector::new("app.popup.delayed");
  pub const OPEN_NEXT: Selector<Popup> = Selector::new("app.popup.open_next");

  pub fn overlay(widget: impl Widget<App> + 'static) -> impl Widget<App> {
    Mask::new(widget)
      .with_mask(Align::centered(Popup::view()))
      .dynamic(|data, _| !data.popups.is_empty())
      .on_command(Popup::OPEN_POPUP, |_, popup, data| {
        data.popups.push_front(popup.clone())
      })
      .on_command(Popup::QUEUE_POPUP, |_, popup, data| {
        data.popups.push_back(popup.clone())
      })
      .on_command(Popup::DISMISS, |_, _, data| {
        data.popups.pop_front();
      })
      .on_command(Popup::DISMISS_MATCHING, |_, matching, data| {
        data.popups.retain(|popup| !matching(popup))
      })
      .scope_with((DataTimer::INVALID, Vector::new()), |scoped| {
        scoped
          .on_command(Popup::DELAYED_POPUP, |ctx, popups, data| {
            let data = &mut data.inner;
            data.0 = ctx.request_timer(std::time::Duration::from_nanos(1)).into();
            data.1.append(popups.into());
          })
          .on_event(|_, _, event, data| {
            let inner = &mut data.inner;
            if let druid::Event::Timer(token) = event
              && *token == *inner.0
            {
              data.outer.popups.extend(inner.1.clone());

              true
            } else {
              false
            }
          })
      })
      .on_change(|_, old, data: &mut App, _| {
        if !old.settings.show_duplicate_warnings && !data.settings.show_duplicate_warnings {
          data
            .popups
            .retain(|popup| !matches!(popup, Popup::Duplicate(_)))
        }
      })
      .controller(MaskController::new())
  }

  pub fn view() -> impl Widget<App> {
    ViewSwitcher::new(
      |data: &App, _| data.popups.clone(),
      |popups, _, _| {
        if let Some(popup) = &popups.front() {
          match popup {
            Popup::ConfirmDelete(entry) => ConfirmDelete::view(entry).boxed(),
            Popup::SelectInstall => SelectInstall::view().boxed(),
            Popup::Ovewrite(overwrite) => overwrite.view().boxed(),
            Popup::Duplicate(duplicate) => duplicate.view().boxed(),
            Popup::FoundMultiple(multiple) => multiple.view().boxed(),
            Popup::RemoteUpdate(remote_update) => remote_update.view().boxed(),
            Popup::Custom(maker) => maker().constant(()).boxed(),
          }
        } else {
          SizedBox::empty().boxed()
        }
      },
    )
  }

  pub fn overwrite(conflict: StringOrPath, to_install: HybridPath, entry: ModEntry) -> Popup {
    Popup::Ovewrite(Overwrite::new(conflict, to_install, entry))
  }

  pub fn duplicate(duplicates: Vector<ModEntry>) -> Popup {
    Popup::Duplicate(Duplicate::new(duplicates))
  }

  pub fn found_multiple(source: HybridPath, found: Vec<ModEntry>) -> Popup {
    Popup::FoundMultiple(Multiple::new(source, found.into()))
  }

  pub fn remote_update<T>(mod_entry: &ModEntry<T>) -> Popup {
    Popup::RemoteUpdate(RemoteUpdate::new(
      mod_entry.id.clone(),
      mod_entry.name.clone(),
      mod_entry.version.clone(),
      mod_entry.remote_version.clone().unwrap(),
    ))
  }

  pub fn custom(maker: impl Fn() -> Box<dyn Widget<()>> + Send + Sync + 'static) -> Popup {
    Popup::Custom(Arc::new(maker))
  }

  pub fn dismiss_matching(matching: impl Fn(&Popup) -> bool + Send + Sync + 'static) -> Command {
    Self::DISMISS_MATCHING.with(Arc::new(matching))
  }
}

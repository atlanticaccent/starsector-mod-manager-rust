use proc_macros::icon;

mod icons {
  pub use druid_widget_nursery::material_icons::normal::{
    action::{
      DELETE, DONE_ALL, EXTENSION, HELP, INSTALL_DESKTOP, OPEN_IN_BROWSER as OPEN_BROWSER, SEARCH,
      SETTINGS, VERIFIED,
    },
    av::{NEW_RELEASES, PLAY_ARROW},
    content::{
      ADD_BOX, ADD_CIRCLE, ADD_CIRCLE_OUTLINE, CLEAR, CONTENT_COPY, DESELECT, INVENTORY_2, LINK,
      LINK_OFF, REPORT,
    },
    file::FOLDER,
    image::{NAVIGATE_NEXT, TUNE},
    navigation::{
      ARROW_DROP_DOWN, ARROW_DROP_UP, ARROW_LEFT, ARROW_RIGHT, CANCEL, CHEVRON_LEFT, CLOSE,
      FIRST_PAGE, LAST_PAGE, REFRESH, UNFOLD_MORE,
    },
    notification::{SYNC, SYSTEM_UPDATE},
    toggle::{
      CHECK_BOX_OUTLINE_BLANK, INDETERMINATE_CHECK_BOX, RADIO_BUTTON_CHECKED,
      RADIO_BUTTON_UNCHECKED, TOGGLE_ON,
    },
  };
}

pub mod icon;

macro_rules! icons {
  ($($i:path),* $(,)?) => {
    $(icon!($i);)+
  };
}

icons! {
  icons::EXTENSION,
  icons::HELP,
  icons::INSTALL_DESKTOP,
  icons::OPEN_BROWSER,
  icons::SETTINGS,
  icons::VERIFIED,
  icons::NEW_RELEASES,
  icons::PLAY_ARROW,
  icons::ADD_CIRCLE,
  icons::ADD_CIRCLE_OUTLINE,
  icons::INVENTORY_2,
  icons::REPORT,
  icons::NAVIGATE_NEXT,
  icons::ARROW_DROP_DOWN,
  icons::ARROW_DROP_UP,
  icons::ARROW_LEFT,
  icons::ARROW_RIGHT,
  icons::CHEVRON_LEFT,
  icons::CLOSE,
  icons::FIRST_PAGE,
  icons::LAST_PAGE,
  icons::UNFOLD_MORE,
  icons::SYNC,
  icons::FOLDER,
  icons::TOGGLE_ON,
  icons::SYSTEM_UPDATE,
  icons::DELETE,
  icons::SEARCH,
  icons::CANCEL,
  icons::TUNE,
  icons::CHECK_BOX_OUTLINE_BLANK,
  icons::ADD_BOX,
  icons::INDETERMINATE_CHECK_BOX,
  icons::RADIO_BUTTON_CHECKED,
  icons::RADIO_BUTTON_UNCHECKED,
  icons::DESELECT,
  icons::CLEAR,
  icons::REFRESH,
  icons::LINK,
  icons::LINK_OFF,
  icons::CONTENT_COPY,
  icons::DONE_ALL,
}

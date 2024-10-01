use proc_macros::icon;

mod _icons {
  pub use druid_widget_nursery::material_icons::normal::{
    action::{
      BOOKMARK, BOOKMARK_BORDER, DELETE, DONE_ALL, EXTENSION, HELP, INFO, INSTALL_DESKTOP,
      OPEN_IN_BROWSER as OPEN_BROWSER, SEARCH, SETTINGS, THUMB_UP, VERIFIED,
    },
    av::{NEW_RELEASES, PLAY_ARROW, SHUFFLE},
    communication::HOURGLASS_TOP,
    content::{
      ADD_BOX, ADD_CIRCLE, ADD_CIRCLE_OUTLINE, CLEAR, CONTENT_COPY, DESELECT, INVENTORY_2, LINK,
      LINK_OFF, REPORT, SORT,
    },
    file::FOLDER,
    hardware::{
      KEYBOARD_DOUBLE_ARROW_LEFT as DOUBLE_LEFT, KEYBOARD_DOUBLE_ARROW_RIGHT as DOUBLE_RIGHT,
    },
    image::{NAVIGATE_NEXT, TUNE},
    maps::HANDYMAN,
    navigation::{
      ARROW_DROP_DOWN, ARROW_DROP_UP, ARROW_LEFT, ARROW_RIGHT, CANCEL, CHEVRON_LEFT, CHEVRON_RIGHT,
      CLOSE, FIRST_PAGE, LAST_PAGE, REFRESH, UNFOLD_MORE,
    },
    notification::{SYNC, SYSTEM_UPDATE},
    social::{CONSTRUCTION, SICK},
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
  _icons::EXTENSION,
  _icons::HELP,
  _icons::INSTALL_DESKTOP,
  _icons::OPEN_BROWSER,
  _icons::SETTINGS,
  _icons::VERIFIED,
  _icons::NEW_RELEASES,
  _icons::PLAY_ARROW,
  _icons::ADD_CIRCLE,
  _icons::ADD_CIRCLE_OUTLINE,
  _icons::INVENTORY_2,
  _icons::REPORT,
  _icons::NAVIGATE_NEXT,
  _icons::ARROW_DROP_DOWN,
  _icons::ARROW_DROP_UP,
  _icons::ARROW_LEFT,
  _icons::ARROW_RIGHT,
  _icons::CHEVRON_LEFT,
  _icons::CHEVRON_RIGHT,
  _icons::CLOSE,
  _icons::FIRST_PAGE,
  _icons::LAST_PAGE,
  _icons::UNFOLD_MORE,
  _icons::SYNC,
  _icons::FOLDER,
  _icons::TOGGLE_ON,
  _icons::SYSTEM_UPDATE,
  _icons::DELETE,
  _icons::SEARCH,
  _icons::CANCEL,
  _icons::TUNE,
  _icons::CHECK_BOX_OUTLINE_BLANK,
  _icons::ADD_BOX,
  _icons::INDETERMINATE_CHECK_BOX,
  _icons::RADIO_BUTTON_CHECKED,
  _icons::RADIO_BUTTON_UNCHECKED,
  _icons::DESELECT,
  _icons::CLEAR,
  _icons::REFRESH,
  _icons::LINK,
  _icons::LINK_OFF,
  _icons::CONTENT_COPY,
  _icons::DONE_ALL,
  _icons::CONSTRUCTION,
  _icons::HANDYMAN,
  _icons::HOURGLASS_TOP,
  _icons::BOOKMARK,
  _icons::BOOKMARK_BORDER,
  _icons::SORT,
  _icons::DOUBLE_LEFT,
  _icons::DOUBLE_RIGHT,
  _icons::INFO,
  _icons::SHUFFLE,
  _icons::SICK,
  _icons::THUMB_UP
}

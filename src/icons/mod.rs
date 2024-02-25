use proc_macros::icon;

mod icons {
  pub use druid_widget_nursery::material_icons::normal::{
    action::{
      DELETE, EXTENSION, HELP, INSTALL_DESKTOP, OPEN_IN_BROWSER as OPEN_BROWSER, SEARCH, SETTINGS,
      VERIFIED,
    },
    av::{NEW_RELEASES, PLAY_ARROW},
    content::{ADD_BOX, ADD_CIRCLE, ADD_CIRCLE_OUTLINE, CLEAR, DESELECT, INVENTORY_2, REPORT},
    file::FOLDER,
    image::{NAVIGATE_NEXT, TUNE},
    navigation::{
      ARROW_DROP_DOWN, ARROW_DROP_UP, ARROW_LEFT, ARROW_RIGHT, CANCEL, CHEVRON_LEFT, CLOSE,
      FIRST_PAGE, LAST_PAGE, UNFOLD_MORE,
    },
    notification::{SYNC, SYSTEM_UPDATE},
    toggle::{
      CHECK_BOX_OUTLINE_BLANK, INDETERMINATE_CHECK_BOX, RADIO_BUTTON_CHECKED,
      RADIO_BUTTON_UNCHECKED, TOGGLE_ON,
    },
  };
}

pub mod icon;

icon!(icons::EXTENSION);
icon!(icons::HELP);
icon!(icons::INSTALL_DESKTOP);
icon!(icons::OPEN_BROWSER);
icon!(icons::SETTINGS);
icon!(icons::VERIFIED);
icon!(icons::NEW_RELEASES);
icon!(icons::PLAY_ARROW);
icon!(icons::ADD_CIRCLE);
icon!(icons::ADD_CIRCLE_OUTLINE);
icon!(icons::INVENTORY_2);
icon!(icons::REPORT);
icon!(icons::NAVIGATE_NEXT);
icon!(icons::ARROW_DROP_DOWN);
icon!(icons::ARROW_DROP_UP);
icon!(icons::ARROW_LEFT);
icon!(icons::ARROW_RIGHT);
icon!(icons::CHEVRON_LEFT);
icon!(icons::CLOSE);
icon!(icons::FIRST_PAGE);
icon!(icons::LAST_PAGE);
icon!(icons::UNFOLD_MORE);
icon!(icons::SYNC);
icon!(icons::FOLDER);
icon!(icons::TOGGLE_ON);
icon!(icons::SYSTEM_UPDATE);
icon!(icons::DELETE);
icon!(icons::SEARCH);
icon!(icons::CANCEL);
icon!(icons::TUNE);
icon!(icons::CHECK_BOX_OUTLINE_BLANK);
icon!(icons::ADD_BOX);
icon!(icons::INDETERMINATE_CHECK_BOX);
icon!(icons::RADIO_BUTTON_CHECKED);
icon!(icons::RADIO_BUTTON_UNCHECKED);
icon!(icons::DESELECT);
icon!(icons::CLEAR);

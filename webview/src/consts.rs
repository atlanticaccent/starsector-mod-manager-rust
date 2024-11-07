use std::sync::LazyLock;

use directories::ProjectDirs;
use druid::Selector;

use crate::{InstallType, WebviewEvent};

pub static PROJECT: LazyLock<ProjectDirs> = LazyLock::new(|| {
  ProjectDirs::from("org", "laird", "Starsector Mod Manager").expect("Get project dirs")
});

pub const FRACTAL_INDEX: &str = "https://fractalsoftworks.com/forum/index.php?topic=177.0";
pub const FRACTAL_MODS_FORUM: &str = "https://fractalsoftworks.com/forum/index.php?board=8.0";
pub const FRACTAL_MODDING_SUBFORUM: &str = "https://fractalsoftworks.com/forum/index.php?board=3.0";

pub const WEBVIEW_EVENT: Selector<WebviewEvent> = Selector::new("webview.event");
pub const WEBVIEW_INSTALL: Selector<InstallType> = Selector::new("webview.install");

pub const WEBVIEW_OFFSET: i16 = 34;

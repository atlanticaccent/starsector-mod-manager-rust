#![feature(let_chains)]
#![feature(result_flattening)]
#![feature(try_blocks)]

pub(crate) mod installer;
mod traits;
mod error;

pub use installer::{HybridPath, Request, StringOrPath};
pub use traits::{Entry, EntryUpdate, InstallerDelegate, InstallerExt};
pub use error::InstallError;

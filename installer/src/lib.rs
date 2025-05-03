#![feature(let_chains)]
#![feature(result_flattening)]
#![feature(try_blocks)]

mod error;
pub(crate) mod installer;
mod traits;

pub use error::InstallError;
pub use installer::{EnrichedEntry, HybridPath, Request, StringOrPath};
pub use traits::{Entry, EntryUpdate, InstallerDelegate, InstallerExt};

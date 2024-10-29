#![feature(let_chains)]
#![feature(result_flattening)]

pub(crate) mod installer;
mod traits;

pub use installer::{HybridPath, Request, StringOrPath};
pub use traits::{Entry, InstallerDelegate, InstallerExt};

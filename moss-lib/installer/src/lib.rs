#![feature(let_chains)]

mod installer;
mod traits;

pub use installer::{HybridPath, Installer, StringOrPath};
pub use traits::{Entry, InstallerDelegate};

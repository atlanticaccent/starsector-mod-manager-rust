#![feature(
  maybe_uninit_slice,
  maybe_uninit_uninit_array_transpose,
  maybe_uninit_write_slice
)]

mod array_set;
mod clone_tx;

pub use array_set::*;
pub use clone_tx::*;

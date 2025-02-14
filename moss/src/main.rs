#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit = "1000"]
// Nightly features
#![feature(option_zip)]
#![feature(result_flattening)]
#![feature(hash_set_entry)]
#![feature(string_remove_matches)]
#![feature(io_error_more)]
#![feature(try_blocks)]
#![feature(let_chains)]
#![feature(iterator_try_collect)]
#![feature(iter_next_chunk)]
#![feature(test)]
#![feature(cfg_match)]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![feature(map_try_insert)]
#![feature(extract_if)]
#![feature(linked_list_cursors)]
#![feature(default_field_values)]
#![feature(get_many_mut)]
// Ignored lints
#![allow(clippy::new_ret_no_self)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::if_not_else)]

#[cfg(feature = "leaky-api")]
pub mod app;
#[cfg(not(feature = "leaky-api"))]
pub(crate) mod app;
pub(crate) mod entrypoint;
pub(crate) mod formatter;
pub(crate) mod nav_bar;
pub(crate) mod theme;

pub(crate) const ENV_STATE: druid::Key<std::sync::Arc<app::EnvSharedData>> =
  druid::Key::new("global.env_shared_state");

pub(crate) mod widgets {
  use common::widgets::root_stack::RootStack as GenericRootStack;

  use crate::app::App;

  pub(crate) type RootStack = GenericRootStack<App>;
}

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
  entrypoint::start();
}

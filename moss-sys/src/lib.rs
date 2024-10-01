#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit = "1000"]
#![feature(option_zip)]
#![feature(result_flattening)]
#![feature(async_closure)]
#![feature(hash_set_entry)]
#![feature(string_remove_matches)]
#![feature(io_error_more)]
#![feature(try_blocks)]
#![feature(let_chains)]
#![feature(iterator_try_collect)]
#![feature(iter_next_chunk)]
#![feature(test)]
#![feature(const_collections_with_hasher)]
#![feature(cfg_match)]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]

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
pub(crate) mod formatter;
pub(crate) mod nav_bar;
#[cfg(feature = "leaky-api")]
#[allow(dead_code)]
pub mod patch;
#[cfg(not(feature = "leaky-api"))]
#[allow(dead_code)]
pub(crate) mod patch;
pub(crate) mod theme;
#[allow(dead_code)]
pub(crate) mod widgets;
pub mod entrypoint;

pub(crate) const ENV_STATE: druid::Key<std::sync::Arc<app::EnvSharedData>> =
  druid::Key::new("global.env_shared_state");

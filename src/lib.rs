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
#![allow(clippy::new_ret_no_self)]

pub mod app;
pub mod formatter;
pub mod nav_bar;
#[allow(dead_code)]
pub mod patch;
pub mod theme;
#[allow(dead_code)]
pub mod widgets;

pub use app::EnvSharedData;

pub const ENV_STATE: druid::Key<std::sync::Arc<EnvSharedData>> =
  druid::Key::new("global.env_shared_state");

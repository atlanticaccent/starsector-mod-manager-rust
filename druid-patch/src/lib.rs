#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]

pub mod click;
pub mod separator;
pub mod split;
#[allow(unused)]
pub mod table;
#[allow(unused)]
pub mod tabs;
pub mod tooltip;
pub mod tree;
pub mod switch;

pub use tabs::tabs_policy;

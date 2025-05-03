mod deser;
mod error;
mod parser;
mod traits;
mod types;

pub use deser::from_str;
pub use parser::JsonParser;
pub use traits::CommentDefinition;

mod sealed {
  pub(super) trait Sealed {}

  impl<const S: usize, const L: usize, T> Sealed for super::JsonParser<S, L, T> {}
}

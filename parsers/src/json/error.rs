use std::fmt::Display;

use winnow::error::{ContextError, ErrMode};

pub(crate) type ModalError = ErrMode<ContextError>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("Remaining input after parsing")]
  TrailingCharacters,
  #[error("Parse error: {0:?}")]
  Parsing(ModalError),
  #[error("Custom: {0}")]
  Custom(String),
}

impl From<ModalError> for Error {
  fn from(value: ModalError) -> Self {
    Self::Parsing(value)
  }
}

impl serde::de::Error for Error {
  #[doc = r" Raised when there is general error when deserializing a type."]
  #[doc = r""]
  #[doc = r" The message should not be capitalized and should not end with a period."]
  #[doc = r""]
  #[doc = r" ```edition2021"]
  #[doc = r" # use std::str::FromStr;"]
  #[doc = r" #"]
  #[doc = r" # struct IpAddr;"]
  #[doc = r" #"]
  #[doc = r" # impl FromStr for IpAddr {"]
  #[doc = r" #     type Err = String;"]
  #[doc = r" #"]
  #[doc = r" #     fn from_str(_: &str) -> Result<Self, String> {"]
  #[doc = r" #         unimplemented!()"]
  #[doc = r" #     }"]
  #[doc = r" # }"]
  #[doc = r" #"]
  #[doc = r" use serde::de::{self, Deserialize, Deserializer};"]
  #[doc = r""]
  #[doc = r" impl<'de> Deserialize<'de> for IpAddr {"]
  #[doc = r"     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>"]
  #[doc = r"     where"]
  #[doc = r"         D: Deserializer<'de>,"]
  #[doc = r"     {"]
  #[doc = r"         let s = String::deserialize(deserializer)?;"]
  #[doc = r"         s.parse().map_err(de::Error::custom)"]
  #[doc = r"     }"]
  #[doc = r" }"]
  #[doc = r" ```"]
  fn custom<T>(msg: T) -> Self
  where
    T: Display,
  {
    Self::Custom(msg.to_string())
  }
}

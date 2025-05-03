use std::{
  cell::{RefCell, RefMut},
  fmt::Display,
  str::FromStr,
};

use druid::text::{Formatter, Selection, Validation, ValidationError};

pub(crate) struct ParseOrLastFormatter<T> {
  pub(crate) previous: RefCell<Option<T>>,
}

impl<T> ParseOrLastFormatter<T> {
  pub(crate) fn new() -> Self {
    Self {
      previous: RefCell::new(None),
    }
  }

  pub(crate) fn get_previous(&self) -> RefMut<Option<T>> {
    self.previous.borrow_mut()
  }
}

impl<T> Formatter<T> for ParseOrLastFormatter<T>
where
  T: FromStr + Display + Clone,
  <T as FromStr>::Err: std::error::Error + 'static,
{
  fn format(&self, value: &T) -> String {
    self.get_previous().replace(value.clone());
    value.to_string()
  }

  fn validate_partial_input(&self, input: &str, _sel: &Selection) -> Validation {
    match input.parse::<T>() {
      Ok(val) => {
        self.get_previous().replace(val);
        Validation::success()
      }
      Err(e) => {
        if self.get_previous().is_some() {
          Validation::success()
        } else {
          Validation::failure(e)
        }
      }
    }
  }

  fn value(&self, input: &str) -> Result<T, ValidationError> {
    match input.parse::<T>() {
      Ok(val) => {
        self.get_previous().replace(val.clone());
        Ok(val)
      }
      Err(_) if self.get_previous().is_some() => Ok(self.get_previous().clone().unwrap()),
      Err(err) => Err(ValidationError::new(err)),
    }
  }
}

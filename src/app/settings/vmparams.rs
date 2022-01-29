use std::fmt::Display;
use std::str::Chars;
use std::iter::Peekable;
use std::path::PathBuf;
use druid::{Data, Lens};
use if_chain::if_chain;

use crate::app::util::{LoadError, SaveError};

#[derive(Debug, Clone, Data, Lens)]
pub struct VMParams {
  pub heap_init: Value,
  pub heap_max: Value,
  pub thread_stack_size: Value
}

#[derive(Debug, Clone, Data, Lens)]
pub struct Value {
  pub amount: i32,
  pub unit: Unit
}

impl Value {
  pub fn to_string(&self) -> String {
    let mut output = self.amount.to_string();
    output.push(self.unit.to_char());

    output
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Data)]
pub enum Unit {
  Giga,
  Mega,
  Kilo
}

impl Unit {
  pub const ALL: [Unit; 3] = [
    Unit::Kilo,
    Unit::Mega,
    Unit::Giga,
  ];

  pub fn to_char(&self) -> char {
    match self {
      Unit::Giga => 'g',
      Unit::Mega => 'm',
      Unit::Kilo => 'k',
    }
  }
}

impl Default for Unit {
  fn default() -> Unit {
    Unit::Kilo
  }
}

impl Display for Unit {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    write!(f, "{}", match self {
      Unit::Giga => "G",
      Unit::Mega => "M",
      Unit::Kilo => "K"
    })
  }
}

impl VMParams {
  fn path() -> PathBuf {
    PathBuf::from(r"./vmparams")
  }

  pub fn load(install_dir: PathBuf) -> Result<VMParams, LoadError> {
    use std::fs;
    use std::io::Read;

    let mut params_file = fs::File::open(install_dir.join(VMParams::path()))
      .map_err(|_| LoadError::NoSuchFile)?;

    let mut params_string = String::new();
    params_file.read_to_string(&mut params_string)
      .map_err(|_| LoadError::ReadError)?;

    let (mut heap_init, mut heap_max, mut thread_stack_size) = (None, None, None);
    for param in params_string.split_ascii_whitespace() {
      let unit = || -> Option<Unit> {
        match param.chars().last() {
          Some('k') | Some('K') => Some(Unit::Kilo),
          Some('m') | Some('M') => Some(Unit::Mega),
          Some('g') | Some('G') => Some(Unit::Giga),
          Some(_) | None => None
        }
      };
      let amount = || -> Result<i32, LoadError> {
        let val = &param[4.. param.len() - 1].to_string().parse::<i32>();
        val.clone().map_err(|_| LoadError::FormatError)
      };
      let parse_pair = || -> Result<Option<Value>, LoadError> {
        if let Some(unit) = unit() {
          Ok(Some(Value {
            amount: amount()?,
            unit
          }))
        } else {
          Err(LoadError::FormatError)
        }
      };

      match &param[..4] {
        "-Xms" | "-xms" => heap_init = parse_pair()?,
        "-Xmx" | "-xmx" => heap_max = parse_pair()?,
        "-Xss" | "-xss" => thread_stack_size = parse_pair()?,
        _ => {}
      }
    }

    if let (Some(heap_init), Some(heap_max), Some(thread_stack_size)) = (heap_init, heap_max, thread_stack_size) {
      Ok(VMParams {
        heap_init,
        heap_max,
        thread_stack_size
      })
    } else {
      Err(LoadError::FormatError)
    }
  }

  pub fn save(self, install_dir: PathBuf) -> Result<(), SaveError> {
    use std::fs;
    use std::io::{Read, Write};

    let mut params_file = fs::File::open(install_dir.join(VMParams::path()))
      .map_err(|_| SaveError::FormatError)?;

    let mut params_string = String::new();
    params_file.read_to_string(&mut params_string)
      .map_err(|_| SaveError::FormatError)?;

    let mut output = String::new();
    let mut input_iter = params_string.chars().peekable();
    while let Some(ch) = input_iter.peek().cloned() {
      match ch {
        '-' => {
          match input_iter.clone().take(4).collect::<String>().as_str() {
            key @ "-Xms" | key @ "-xms" => {
              VMParams::consume_value(&mut input_iter)?;
              output.push_str(key);
              output.push_str(&self.heap_init.to_string())
            },
            key @ "-Xmx" | key @ "-xmx" => {
              VMParams::consume_value(&mut input_iter)?;
              output.push_str(key);
              output.push_str(&self.heap_max.to_string())
            },
            key @ "-Xss" | key @ "-xss" => {
              VMParams::consume_value(&mut input_iter)?;
              output.push_str(key);
              output.push_str(&self.thread_stack_size.to_string())
            },
            _ => {
              output.push(ch);
              input_iter.next();
            }
          }
        },
        _ => {
          if let Some(next) = input_iter.next() {
            output.push(next)
          }
        },
      }
    };

    let mut file = fs::File::create(install_dir.join(VMParams::path()))
      .map_err(|_| SaveError::FileError)?;

    file.write_all(output.as_bytes())
      .map_err(|_| SaveError::WriteError)
  }

  /**
   * Specify a pattern for the value in the paramter pair, then attempt to 
   * consume - if the pattern is not met throw error.
   * Pattern is [any number of digits][k | K | m | M | g | G][space | EOF]
   */
  fn consume_value(iter: &mut Peekable<Chars>) -> Result<(), SaveError> {
    iter.nth(3);

    let mut count = 0;
    while let Some(ch) = iter.peek() {
      if ch.is_numeric() {
        count = count + 1;
        iter.next();
      } else {
        break;
      }
    };

    if_chain! {
      if count > 0;
      if let Some(ch) = iter.next();
      if vec!['k', 'K', 'm', 'M', 'g', 'G'].iter().any(|t| *t == ch);
      if let Some(' ') | None = iter.peek();
      then {
        Ok(())
      } else {
        Err(SaveError::FormatError)
      }
    }
  }
}

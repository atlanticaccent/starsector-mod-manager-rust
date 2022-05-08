use druid::{Data, Lens};
use if_chain::if_chain;
use std::iter::Peekable;
use std::marker::PhantomData;
use std::path::PathBuf;
use std::str::Chars;
use std::{fmt::Display, path::Path};
use strum_macros::EnumIter;

use crate::app::util::{LoadError, SaveError};

#[derive(Debug, Clone, Data, Lens)]
pub struct VMParams<T = VMParamsPathDefault> {
  pub heap_init: Value,
  pub heap_max: Value,
  pub thread_stack_size: Value,
  _phantom: PhantomData<T>,
}

#[derive(Debug, Clone, Data, Lens)]
pub struct Value {
  pub amount: i32,
  pub unit: Unit,
}

impl Display for Value {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{}{}", self.amount, self.unit))
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Data, EnumIter)]
pub enum Unit {
  Giga,
  Mega,
  Kilo,
}

impl Default for Unit {
  fn default() -> Unit {
    Unit::Kilo
  }
}

impl Display for Unit {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
    write!(
      f,
      "{}",
      match self {
        Unit::Giga => "G",
        Unit::Mega => "M",
        Unit::Kilo => "K",
      }
    )
  }
}

pub trait VMParamsPath {
  fn path() -> PathBuf {
    #[cfg(target_os = "windows")]
    return PathBuf::from(r"./vmparams");
    #[cfg(target_os = "macos")]
    return PathBuf::from("./Contents/MacOS/starsector_mac.sh");
    #[cfg(target_os = "linux")]
    return PathBuf::from("./starsector.sh")
  }
}

#[derive(Debug, Clone, Data)]
pub struct VMParamsPathDefault;

impl VMParamsPath for VMParamsPathDefault {}

impl<T: VMParamsPath> VMParams<T> {
  pub fn load(install_dir: impl AsRef<Path>) -> Result<VMParams<T>, LoadError> {
    use std::fs;
    use std::io::Read;

    let mut params_file =
      fs::File::open(install_dir.as_ref().join(T::path())).map_err(|_| LoadError::NoSuchFile)?;

    let mut params_string = String::new();
    params_file
      .read_to_string(&mut params_string)
      .map_err(|_| LoadError::ReadError)?;

    let (mut heap_init, mut heap_max, mut thread_stack_size) = (None, None, None);
    for param in params_string.split_ascii_whitespace() {
      let unit = || -> Option<Unit> {
        match param.chars().last() {
          Some('k') | Some('K') => Some(Unit::Kilo),
          Some('m') | Some('M') => Some(Unit::Mega),
          Some('g') | Some('G') => Some(Unit::Giga),
          Some(_) | None => None,
        }
      };
      let amount = || -> Result<i32, LoadError> {
        let val = &param[4..param.len() - 1].to_string().parse::<i32>();
        val.clone().map_err(|_| LoadError::FormatError)
      };
      let parse_pair = || -> Result<Option<Value>, LoadError> {
        if let Some(unit) = unit() {
          Ok(Some(Value {
            amount: amount()?,
            unit,
          }))
        } else {
          Err(LoadError::FormatError)
        }
      };

      if let Some(slice) = param.get(..4) {
        match slice {
          "-Xms" | "-xms" => heap_init = parse_pair()?,
          "-Xmx" | "-xmx" => heap_max = parse_pair()?,
          "-Xss" | "-xss" => thread_stack_size = parse_pair()?,
          _ => {}
        }
      }
    }

    if let (Some(heap_init), Some(heap_max), Some(thread_stack_size)) =
      (heap_init, heap_max, thread_stack_size)
    {
      Ok(VMParams {
        heap_init,
        heap_max,
        thread_stack_size,
        _phantom: PhantomData::default(),
      })
    } else {
      Err(LoadError::FormatError)
    }
  }

  pub fn save(self, install_dir: impl AsRef<Path>) -> Result<(), SaveError> {
    use std::fs;
    use std::io::{Read, Write};

    let mut params_file =
      fs::File::open(install_dir.as_ref().join(T::path())).map_err(|_| SaveError::Format)?;

    let mut params_string = String::new();
    params_file
      .read_to_string(&mut params_string)
      .map_err(|_| SaveError::Format)?;

    let mut output = String::new();
    let mut input_iter = params_string.chars().peekable();
    while let Some(ch) = input_iter.peek().cloned() {
      match ch {
        '-' => match input_iter.clone().take(4).collect::<String>().as_str() {
          key @ "-Xms" | key @ "-xms" => {
            VMParams::<T>::consume_value(&mut input_iter)?;
            output.push_str(key);
            output.push_str(&self.heap_init.to_string())
          }
          key @ "-Xmx" | key @ "-xmx" => {
            VMParams::<T>::consume_value(&mut input_iter)?;
            output.push_str(key);
            output.push_str(&self.heap_max.to_string())
          }
          key @ "-Xss" | key @ "-xss" => {
            VMParams::<T>::consume_value(&mut input_iter)?;
            output.push_str(key);
            output.push_str(&self.thread_stack_size.to_string())
          }
          _ => {
            output.push(ch);
            input_iter.next();
          }
        },
        _ => {
          if let Some(next) = input_iter.next() {
            output.push(next)
          }
        }
      }
    }

    let mut file =
      fs::File::create(install_dir.as_ref().join(T::path())).map_err(|_| SaveError::File)?;

    file
      .write_all(output.as_bytes())
      .map_err(|_| SaveError::Write)
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
        count += 1;
        iter.next();
      } else {
        break;
      }
    }

    if_chain! {
      if count > 0;
      if let Some(ch) = iter.next();
      if vec!['k', 'K', 'm', 'M', 'g', 'G'].iter().any(|t| *t == ch);
      if let Some(' ') | None = iter.peek();
      then {
        Ok(())
      } else {
        Err(SaveError::Format)
      }
    }
  }
}

#[cfg(test)]
mod test {
  use std::{io::Seek, marker::PhantomData, path::PathBuf, sync::Mutex};

  use crate::app::settings::vmparams::{VMParamsPath, VMParams};

  lazy_static::lazy_static! {
    static ref TEST_FILE: tempfile::NamedTempFile = tempfile::NamedTempFile::new().expect("Couldn't create tempdir - not a real test failure");

    static ref ROOT: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("assets");

    static ref DUMB_MUTEX: Mutex<()> = Mutex::new(());
  }

  struct TempPath;

  impl VMParamsPath for TempPath {
    fn path() -> PathBuf {
      TEST_FILE.path().to_path_buf()
    }
  }

  fn test_func<T: VMParamsPath>() {
    let _guard = DUMB_MUTEX.lock().expect("Lock dumb mutex");

    let root = ROOT.as_path();

    let vmparams = VMParams::<T>::load(root);

    assert!(vmparams.is_ok());

    if let Ok(mut vmparams) = vmparams {
      println!(
        "{:?} {:?} {:?}",
        vmparams.heap_init, vmparams.heap_max, vmparams.thread_stack_size
      );

      vmparams.heap_init.amount = 2048;
      vmparams.heap_max.amount = 2048;
      vmparams.thread_stack_size.amount = 4096;

      let mut reader =
        std::fs::File::open(root.join(T::path())).expect("Open original file for reading");

      let mut testfile = TEST_FILE.as_file().clone();
      testfile.set_len(0).expect("Truncate");
      testfile.rewind().expect("Truncate");

      std::io::copy(&mut reader, &mut testfile).expect("Copy vmparams to tempfile");

      let edited_vmparams = VMParams::<TempPath> {
        heap_init: vmparams.heap_init,
        heap_max: vmparams.heap_max,
        thread_stack_size: vmparams.thread_stack_size,
        _phantom: PhantomData::default(),
      };

      let res = edited_vmparams.save(PathBuf::from("/"));

      assert!(res.is_ok());

      let edited_vmparams = VMParams::<TempPath>::load(PathBuf::from("/"));

      assert!(edited_vmparams.is_ok());

      if let Ok(edited_vmparams) = edited_vmparams {
        assert!(edited_vmparams.heap_init.amount == 2048);
        assert!(edited_vmparams.heap_max.amount == 2048);
        assert!(edited_vmparams.thread_stack_size.amount == 4096);
      }
    }
  }

  #[test]
  fn test_windows() {
    struct Windows;

    impl VMParamsPath for Windows {
      fn path() -> PathBuf {
        PathBuf::from("./vmparams_windows")
      }
    }

    test_func::<Windows>();
  }

  #[test]
  fn test_linux() {
    struct Linux;

    impl VMParamsPath for Linux {
      fn path() -> PathBuf {
        PathBuf::from("./vmparams_linux")
      }
    }

    test_func::<Linux>();
  }

  #[test]
  fn test_macos() {
    struct MacOS;

    impl VMParamsPath for MacOS {
      fn path() -> PathBuf {
        PathBuf::from("./vmparams_macos")
      }
    }

    test_func::<MacOS>();
  }
}

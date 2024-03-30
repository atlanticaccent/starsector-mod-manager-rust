use std::{
  fmt::Display,
  iter::Peekable,
  marker::PhantomData,
  path::{Path, PathBuf},
  str::Chars,
  sync::LazyLock,
};

use druid::{
  lens,
  widget::{Either, Flex, Label, Maybe, Painter, SizedBox, TextBox, ViewSwitcher},
  Data, Lens, LensExt, Selector, Widget, WidgetExt as _,
};
use druid_widget_nursery::{material_icons::Icon, wrap::Wrap, WidgetExt};
use regex::{Captures, Regex, RegexBuilder};
use strum_macros::EnumIter;

use crate::{
  app::{
    util::{
      h2_fixed, LoadError, RootStack, SaveError, ShadeColor, WidgetExtEx, WithHoverState as _,
    },
    ARROW_DROP_DOWN, ARROW_LEFT, LINK, LINK_OFF,
  },
  widgets::card::Card,
};

use super::tool_card;

#[derive(Debug, Clone, Data, Lens)]
pub struct VMParams<T: VMParamsPath = VMParamsPathDefault> {
  pub heap_init: Value,
  pub heap_max: Value,
  pub thread_stack_size: Value,
  pub verify_none: bool,
  pub linked: bool,
  _phantom: PhantomData<T>,
}

impl VMParams {
  const TOGGLE_UNIT_DROP: Selector<bool> = Selector::new("vmparams.toggle_unit_dropdown");
  pub const SAVE_VMPARAMS: Selector = Selector::new("vmparams.save");

  pub fn view() -> impl Widget<Self> {
    tool_card()
      .build(
        Flex::column()
          .with_child(h2_fixed("VMParams Editor"))
          .with_child(Label::new(
            "This tool allows you to modify the amount of RAM Starsector is allowed to use.",
          ))
          .with_child(
            Wrap::new()
              .with_child(
                Flex::row()
                  .with_child(
                    TextBox::new()
                      .with_placeholder("Minimum")
                      .with_formatter(ValueFormatter)
                      .update_data_while_editing(true)
                      .lens(Value::amount)
                      .padding((0.0, 4.0)),
                  )
                  .with_child(
                    Card::builder()
                      .with_insets(4.0)
                      .with_shadow_increase(3.0)
                      .with_shadow_length(0.0)
                      .with_border(1.0, druid::Color::BLACK)
                      .with_background(druid::Color::GRAY.lighter_by(9))
                      .hoverable(|| {
                        Flex::row()
                          .with_child(
                            Label::dynamic(|unit: &Unit, _| format!("{}b", unit))
                              .padding((0.0, 2.0)),
                          )
                          .with_child(Icon::new(*ARROW_LEFT))
                          .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
                          .must_fill_main_axis(true)
                          .expand_width()
                      })
                      .fix_width(56.0)
                      .lens(lens!((Unit, bool), 0))
                      .on_click(|ctx, data, _| {
                        data.1 = !data.1;
                        if data.1 {
                          Self::unit_dropdown(ctx, VMParams::heap_init, false);
                        }
                      })
                      .on_command(Self::TOGGLE_UNIT_DROP, |_, payload, data| {
                        if !*payload {
                          data.1 = false
                        }
                      })
                      .invisible_if(|(_, data)| *data)
                      .disabled_if(|(_, data), _| *data)
                      .scope(|unit| (unit, false), lens!((Unit, bool), 0))
                      .lens(Value::unit),
                  )
                  .lens(VMParams::heap_init)
                  .on_change(|ctx, old, data, _| {
                    if data.linked && old.heap_init != data.heap_init {
                      data.heap_max = data.heap_init.clone();
                      ctx.request_update();
                    }
                  }),
              )
              .with_child(SizedBox::empty().fix_width(10.))
              .with_child(
                Either::new(
                  |data: &Self, _| data.linked,
                  Icon::new(*LINK),
                  Icon::new(*LINK_OFF),
                )
                .on_click(|_, data, _| {
                  data.linked = !data.linked;
                  if data.linked {
                    data.heap_max = data.heap_init.clone();
                  }
                })
                .lens(lens!((Self, bool), 0))
                .with_hover_state(false),
              )
              .with_child(SizedBox::empty().fix_width(10.))
              .with_child(
                Flex::row()
                  .with_child(
                    TextBox::new()
                      .with_placeholder("Maximum")
                      .with_formatter(ValueFormatter)
                      .update_data_while_editing(true)
                      .lens(Value::amount)
                      .padding((0.0, 4.0)),
                  )
                  .with_child(
                    Card::builder()
                      .with_insets(4.0)
                      .with_shadow_increase(3.0)
                      .with_shadow_length(0.0)
                      .with_border(1.0, druid::Color::BLACK)
                      .with_background(druid::Color::GRAY.lighter_by(9))
                      .hoverable(|| {
                        Flex::row()
                          .with_child(
                            Label::dynamic(|unit: &Unit, _| format!("{}b", unit))
                              .padding((0.0, 2.0)),
                          )
                          .with_child(Icon::new(*ARROW_LEFT))
                          .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
                          .must_fill_main_axis(true)
                          .expand_width()
                      })
                      .fix_width(56.0)
                      .lens(lens!((Unit, bool), 0))
                      .on_click(|ctx, data, _| {
                        data.1 = !data.1;
                        if data.1 {
                          Self::unit_dropdown(ctx, VMParams::heap_max, true);
                        }
                      })
                      .on_command(Self::TOGGLE_UNIT_DROP, |_, payload, data| {
                        if *payload {
                          data.1 = false
                        }
                      })
                      .invisible_if(|(_, data)| *data)
                      .disabled_if(|(_, data), _| *data)
                      .scope(|unit| (unit, false), lens!((Unit, bool), 0))
                      .lens(Value::unit),
                  )
                  .lens(VMParams::heap_max)
                  .disabled_if(|data, _| data.linked),
              )
              .cross_alignment(druid_widget_nursery::wrap::WrapCrossAlignment::Center),
          )
          .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
          .padding((Card::CARD_INSET, 0.0)),
      )
      .expand_width()
      .on_command(Self::TOGGLE_UNIT_DROP, |ctx, payload, vmparams| {
        if vmparams.linked && !*payload {
          vmparams.heap_max = vmparams.heap_init.clone()
        }
        ctx.submit_notification(Self::SAVE_VMPARAMS)
      })
  }

  fn unit_dropdown(
    ctx: &mut druid::EventCtx,
    lens: impl Lens<VMParams, Value> + Clone + 'static,
    max: bool,
  ) {
    RootStack::show(
      ctx,
      ctx.window_origin(),
      move || {
        let lens = lens.clone();
        Maybe::or_empty(move || {
          Card::builder()
            .with_insets(4.0)
            .with_shadow_increase(3.0)
            .with_shadow_length(0.0)
            .with_border(1.0, druid::Color::BLACK)
            .with_background(druid::Color::WHITE.darker_by(1))
            .hoverable(|| {
              Flex::column()
                .with_child(
                  Card::builder()
                    .with_insets(0.0)
                    .with_shadow_length(0.0)
                    .with_border(1.0, druid::Color::BLACK)
                    .with_background(druid::Color::GRAY.lighter_by(9))
                    .build(
                      Flex::row()
                        .with_child(
                          Label::dynamic(|unit: &Unit, _| format!("{}b", unit)).padding((0.0, 2.0)),
                        )
                        .with_child(Icon::new(*ARROW_DROP_DOWN))
                        .main_axis_alignment(druid::widget::MainAxisAlignment::SpaceBetween)
                        .must_fill_main_axis(true)
                        .padding(2.0),
                    )
                    .expand_width()
                    .padding(-2.0),
                )
                .with_spacer(2.0)
                .with_child(other_units_dropdown(true).expand_width())
                .with_spacer(4.)
                .with_child(other_units_dropdown(false).expand_width())
                .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
                .main_axis_alignment(druid::widget::MainAxisAlignment::Start)
                .expand_width()
            })
            .fix_width(56.0)
            .lens(lens.clone().then(Value::unit))
        })
        .lens(crate::app::App::settings.then(crate::app::settings::Settings::vmparams))
        .on_click(move |ctx, _, _| {
          RootStack::dismiss(ctx);
        })
        .boxed()
      },
      Some(move |ctx: &mut druid::EventCtx| ctx.submit_command(Self::TOGGLE_UNIT_DROP.with(max))),
    )
  }
}

fn other_units_dropdown(higher: bool) -> impl Widget<Unit> {
  let maker = |unit: Unit| {
    Card::builder()
      .with_insets(0.0)
      .with_shadow_length(0.0)
      .with_background(druid::Color::WHITE.darker_by(1))
      .hoverable_distinct(
        || {
          Flex::row()
            .with_child(Label::new(format!("{}b", unit)).padding((0.0, 2.0)))
            .must_fill_main_axis(true)
            .padding(2.0)
            .padding(-1.)
        },
        || {
          Flex::row()
            .with_child(Label::new(format!("{}b", unit)).padding((0.0, 2.0)))
            .must_fill_main_axis(true)
            .padding(2.0)
            .background(Painter::new(|ctx, _, _| {
              use druid::RenderContext;

              let path = ctx.size().to_rect().inset(-0.5).to_rounded_rect(3.);

              ctx.stroke(path, &druid::Color::BLACK, 1.)
            }))
            .padding(-1.)
        },
      )
      .expand_width()
      .on_click(move |_, data, _| *data = unit)
      .boxed()
  };

  ViewSwitcher::new(
    |data, _| *data,
    move |data, _, _| match data {
      Unit::Giga if higher => maker(Unit::Mega),
      Unit::Giga => maker(Unit::Kilo),
      Unit::Mega if higher => maker(Unit::Giga),
      Unit::Mega => maker(Unit::Kilo),
      Unit::Kilo if higher => maker(Unit::Giga),
      Unit::Kilo => maker(Unit::Mega),
    },
  )
}

#[derive(Debug, Clone, Data, Lens, PartialEq)]
pub struct Value {
  pub amount: i32,
  pub unit: Unit,
}

impl Display for Value {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_fmt(format_args!("{}{}", self.amount, self.unit))
  }
}

struct ValueFormatter;

impl druid::text::Formatter<i32> for ValueFormatter {
  fn format(&self, value: &i32) -> String {
    value.to_string()
  }

  fn validate_partial_input(
    &self,
    input: &str,
    _sel: &druid::text::Selection,
  ) -> druid::text::Validation {
    match input.parse::<i32>() {
      Err(err) if input.len() != 0 => druid::text::Validation::failure(err),
      _ => druid::text::Validation::success(),
    }
  }

  fn value(&self, input: &str) -> Result<i32, druid::text::ValidationError> {
    input
      .parse::<i32>()
      .map_err(druid::text::ValidationError::new)
  }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Data, EnumIter)]
pub enum Unit {
  Giga,
  Mega,
  #[default]
  Kilo,
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
    return PathBuf::from("./starsector.sh");
  }
}

#[derive(Debug, Clone, Data)]
pub struct VMParamsPathDefault;

impl VMParamsPath for VMParamsPathDefault {}

static XVERIFY_REGEX: LazyLock<Regex> = LazyLock::new(|| {
  RegexBuilder::new(r"-xverify(?::([^\s]+))?")
    .case_insensitive(true)
    .build()
    .unwrap()
});

impl<T: VMParamsPath> VMParams<T> {
  pub fn load(install_dir: impl AsRef<Path>) -> Result<VMParams<T>, LoadError> {
    use std::{fs, io::Read};

    let mut params_file =
      fs::File::open(install_dir.as_ref().join(T::path())).map_err(|_| LoadError::NoSuchFile)?;

    let mut params_string = String::new();
    params_file
      .read_to_string(&mut params_string)
      .map_err(|_| LoadError::ReadError)?;

    let verify_none = XVERIFY_REGEX
      .captures(&params_string)
      .is_some_and(|captures| {
        captures
          .get(1)
          .is_some_and(|val| val.as_str().eq_ignore_ascii_case("none"))
      });

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
        verify_none,
        linked: false,
        _phantom: PhantomData::default(),
      })
    } else {
      Err(LoadError::FormatError)
    }
  }

  pub fn save(&self, install_dir: impl AsRef<Path>) -> Result<(), SaveError> {
    use std::{
      fs,
      io::{Read, Write},
    };

    let mut params_file =
      fs::File::open(install_dir.as_ref().join(T::path())).map_err(|_| SaveError::Format)?;

    let mut params_string = String::new();
    params_file
      .read_to_string(&mut params_string)
      .map_err(|_| SaveError::Format)?;

    let write_verify_manually = if self.verify_none {
      let mut replaced = false;
      XVERIFY_REGEX.replace(&params_string, |_: &Captures| {
        replaced = true;
        "-Xverify:none"
      });
      !replaced
    } else {
      false
    };

    let mut output = String::new();
    let mut input_iter = params_string.chars().peekable();
    while let Some(ch) = input_iter.next() {
      output.push(ch);
      if ch == '-' {
        let key: String = input_iter
          .next_chunk::<3>()
          .map_or_else(|iter| iter.collect(), |arr| arr.iter().collect());

        if write_verify_manually && key.eq_ignore_ascii_case("ser") {
          let rem: String = input_iter
            .next_chunk::<3>()
            .map_or_else(|iter| iter.collect(), |arr| arr.iter().collect());
          if rem.eq_ignore_ascii_case("ver") {
            #[cfg(target_os = "macos")]
            output.push_str(r"Xverify:none \\n\t-");
            #[cfg(not(target_os = "macos"))]
            output.push_str("Xverify:none -");
          }

          output.push_str(&key);
          output.push_str(&rem);
        } else {
          output.push_str(&key);
          if key.eq_ignore_ascii_case("xms") {
            VMParams::<T>::advance(&mut input_iter)?;
            output.push_str(&self.heap_init.to_string())
          } else if key.eq_ignore_ascii_case("xmx") {
            VMParams::<T>::advance(&mut input_iter)?;
            output.push_str(&self.heap_max.to_string())
          } else if key.eq_ignore_ascii_case("xss") {
            VMParams::<T>::advance(&mut input_iter)?;
            output.push_str(&self.thread_stack_size.to_string())
          }
        }
      } else if write_verify_manually && ch == 'j' {
        let chunk: String = input_iter
          .next_chunk::<7>()
          .map_or_else(|iter| iter.collect(), |arr| arr.iter().collect());
        output.push_str(&chunk);

        if chunk == "ava.exe" {
          output.push_str(" -Xverify:none ")
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
  fn advance(iter: &mut Peekable<Chars>) -> Result<(), SaveError> {
    let mut count = 0;
    while let Some(ch) = iter.peek() {
      if ch.is_numeric() {
        count += 1;
        iter.next();
      } else {
        break;
      }
    }

    if count > 0
      && let Some(ch) = iter.next()
      && vec!['k', 'm', 'g']
        .iter()
        .any(|t| t.eq_ignore_ascii_case(&ch))
      && let Some(' ') | None = iter.peek()
    {
      Ok(())
    } else {
      Err(SaveError::Format)
    }
  }
}

#[cfg(test)]
mod test {
  use std::{io::Seek, marker::PhantomData, path::PathBuf, sync::Mutex};

  use super::{VMParams, VMParamsPath};

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

  fn test_func<T: VMParamsPath>(verify_none: bool) {
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

      let mut testfile = TEST_FILE.as_file();
      testfile.set_len(0).expect("Truncate");
      testfile.rewind().expect("Truncate");

      std::io::copy(&mut reader, &mut testfile).expect("Copy vmparams to tempfile");

      let edited_vmparams = VMParams::<TempPath> {
        heap_init: vmparams.heap_init,
        heap_max: vmparams.heap_max,
        thread_stack_size: vmparams.thread_stack_size,
        verify_none,
        linked: vmparams.linked,
        _phantom: PhantomData::default(),
      };

      let res = edited_vmparams.save(PathBuf::from("/"));

      res.expect("Save edited vmparams");

      let edited_vmparams =
        VMParams::<TempPath>::load(PathBuf::from("/")).expect("Load edited vmparams");

      assert!(edited_vmparams.heap_init.amount == 2048);
      assert!(edited_vmparams.heap_max.amount == 2048);
      assert!(edited_vmparams.thread_stack_size.amount == 4096);
      assert!(edited_vmparams.verify_none == verify_none);
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

    test_func::<Windows>(false);
  }

  #[test]
  fn test_linux() {
    struct Linux;

    impl VMParamsPath for Linux {
      fn path() -> PathBuf {
        PathBuf::from("./vmparams_linux")
      }
    }

    test_func::<Linux>(false);
  }

  #[test]
  fn test_macos() {
    struct MacOS;

    impl VMParamsPath for MacOS {
      fn path() -> PathBuf {
        PathBuf::from("./vmparams_macos")
      }
    }

    test_func::<MacOS>(false);
  }

  #[test]
  fn test_azul() {
    struct Azul;

    impl VMParamsPath for Azul {
      fn path() -> PathBuf {
        PathBuf::from("./vmparams_windows")
      }
    }

    test_func::<Azul>(true)
  }
}

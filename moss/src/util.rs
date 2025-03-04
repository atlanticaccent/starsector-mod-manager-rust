use std::{
  any::Any,
  collections::HashMap,
  fmt::Debug,
  future::Future,
  hash::Hash,
  io::Read,
  marker::PhantomData,
  ops::Deref,
  path::PathBuf,
  sync::{Arc, LazyLock, RwLock, Weak},
};

use druid::{
  widget::Maybe, Color, Data, ExtEventSink, KeyOrValue, Selector, Target, TimerToken, Widget,
};
use json_comments::StripComments;
use regex::Regex;
use tokio::{select, sync::mpsc};
use web_client::WebClient;

use crate::app::mod_entry::{AsyncModVersionMetaRes, GameVersion, ModEntry, ModVersionMeta};

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
  #[error("No such file")]
  NoSuchFile,
  #[error("File format error")]
  FormatError,
  #[error("Archive error")]
  ZipError(#[from] zip::result::ZipError),
  #[error("IO error")]
  IoError(#[from] std::io::Error),
  #[error("Serde error")]
  SerdeError(#[from] serde_json::Error),
  #[error("Join error")]
  JoinError(#[from] tokio::task::JoinError),
  #[error("Parsing error: {0}")]
  ParsingError(anyhow::Error),
  #[error("Other error: {0}")]
  Other(#[from] anyhow::Error),
}

#[derive(Debug, Clone)]
pub enum SaveError {
  File,
  Write,
  Format,
}

#[must_use]
pub fn get_quoted_version(
  starsector_version: &(
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
  ),
) -> Option<String> {
  match starsector_version {
    (None, None, None, None) => None,
    (major, minor, patch, rc) => Some(format!(
      "{}.{}{}{}",
      major.clone().unwrap_or_else(|| "0".to_string()),
      minor.clone().unwrap_or_default(),
      patch.clone().map_or_else(String::new, |p| format!(".{p}")),
      rc.clone()
        .map_or_else(String::new, |rc| format!("a-RC{rc}"))
    )),
  }
}

pub fn get_master_version(
  client: &Arc<WebClient>,
  remote_url: String,
) -> impl Future<Output = AsyncModVersionMetaRes> {
  let client = Arc::clone(client);

  async move {
    let remote = client
      .get(remote_url)
      .await
      .map_err(|e| anyhow::anyhow!(e))?;

    if let Ok(stripped) = std::io::read_to_string(StripComments::new(remote.as_bytes()))
      && let Ok(normalized) = handwritten_json::normalize(&stripped)
      && let Ok(remote) = json5::from_str::<ModVersionMeta>(&normalized)
    {
      Ok(remote)
    } else {
      Err(Arc::new(anyhow::anyhow!("Parse error. Payload:\n{remote}")))
    }
  }
}

pub const GET_INSTALLED_STARSECTOR: Selector<Result<GameVersion, LoadError>> =
  Selector::new("util.starsector_version.get");

pub async fn get_starsector_version(ext_ctx: ExtEventSink, install_dir: PathBuf) {
  use classfile_parser::class_parser;
  use regex::bytes::Regex;
  use tokio::{fs, task};

  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
  ))]
  let obf_jar = install_dir.join("starfarer_obf.jar");
  #[cfg(target_os = "windows")]
  let obf_jar = install_dir.join("starsector-core/starfarer_obf.jar");
  #[cfg(target_os = "macos")]
  let obf_jar = install_dir.join("Contents/Resources/Java/starfarer_obf.jar");

  let mut res = task::spawn_blocking(move || {
    let file = std::fs::File::open(obf_jar)?;
    let mut zip = zip::ZipArchive::new(file)?;

    // println!("{:?}", zip.file_names().collect::<Vec<&str>>());

    let mut version_class = zip.by_name("com/fs/starfarer/Version.class")?;

    let mut buf: Vec<u8> = Vec::new();
    version_class.read_to_end(&mut buf)?;

    let (_, class_file) =
      class_parser(&buf).map_err(|err| LoadError::ParsingError(err.to_owned().into()))?;

    let version_string = class_file
      .fields
      .iter()
      .find_map(|f| {
        use classfile_parser::{
          attribute_info::constant_value_attribute_parser, constant_info::ConstantInfo,
        };
        if let ConstantInfo::Utf8(name) = &class_file.const_pool[(f.name_index - 1) as usize]
          && name.utf8_string == "versionOnly"
          && let Ok((_, attr)) =
            constant_value_attribute_parser(&f.attributes.first().unwrap().info)
          && let ConstantInfo::Utf8(utf_const) =
            &class_file.const_pool[attr.constant_value_index as usize]
        {
          Some(utf_const.utf8_string.clone())
        } else {
          None
        }
      })
      .ok_or(LoadError::ParsingError(anyhow::anyhow!("")));

    version_string
  })
  .await
  .map_err(Into::into)
  .flatten();

  if res.is_err() {
    static RE: LazyLock<Regex> =
      LazyLock::new(|| Regex::new(r"Starting Starsector (.*) launcher").unwrap());

    res = fs::read(install_dir.join("starsector-core").join("starsector.log"))
      .await
      .map_err(Into::into)
      .and_then(|file| {
        RE.captures(&file)
          .and_then(|captures| captures.get(1))
          .ok_or(LoadError::FormatError)
          .and_then(|m| {
            String::from_utf8(m.as_bytes().to_vec()).map_err(|_| LoadError::FormatError)
          })
      });
  };

  let parsed = res.map(|text| parse_game_version(&text));

  if ext_ctx
    .submit_command(GET_INSTALLED_STARSECTOR, parsed, Target::Auto)
    .is_err()
  {
    eprintln!("Failed to submit starsector version back to main thread");
  };
}

/**
* Parses a given version into a four-tuple of the assumed components.
* Assumptions:
* - The first component is always EITHER 0 and thus the major component OR it has been omitted and the first component is the minor component
* - If there are two components it is either the major and minor components OR minor and patch OR minor and RC (release candidate)
* - If there are three components it is either the major, minor and patch OR major, minor and RC OR minor, patch and RC
* - If there are four components then the first components MUST be 0 and MUST be the major component, and the following components
     are the minor, patch and RC components
 */
pub fn parse_game_version(
  text: &str,
) -> (
  Option<String>,
  Option<String>,
  Option<String>,
  Option<String>,
) {
  static VERSION_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\.|a-rc|a").unwrap());
  static RELEASE_CANDIDATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)a-rc").unwrap());

  let components: Vec<&str> = VERSION_REGEX
    .split(text)
    .filter(|c| !c.is_empty())
    .collect();

  match components.as_slice() {
    [major, minor] if major == &"0" => {
      // text = format!("{}.{}a", major, minor);
      (
        Some((*major).to_string()),
        Some((*minor).to_string()),
        None,
        None,
      )
    }
    [minor, patch_rc] => {
      // text = format!("0.{}a-RC{}", minor, rc);
      if RELEASE_CANDIDATE.is_match(patch_rc) {
        (
          Some("0".to_string()),
          Some((*minor).to_string()),
          None,
          Some((*patch_rc).to_string()),
        )
      } else {
        (
          Some("0".to_string()),
          Some((*minor).to_string()),
          Some((*patch_rc).to_string()),
          None,
        )
      }
    }
    [major, minor, patch_rc] if major == &"0" => {
      // text = format!("{}.{}a-RC{}", major, minor, rc);
      if RELEASE_CANDIDATE.is_match(patch_rc) {
        (
          Some((*major).to_string()),
          Some((*minor).to_string()),
          None,
          Some((*patch_rc).to_string()),
        )
      } else {
        (
          Some((*major).to_string()),
          Some((*minor).to_string()),
          Some((*patch_rc).to_string()),
          None,
        )
      }
    }
    [minor, patch, rc] => {
      // text = format!("0.{}.{}a-RC{}", minor, patch, rc);
      (
        Some("0".to_string()),
        Some((*minor).to_string()),
        Some((*patch).to_string()),
        Some((*rc).to_string()),
      )
    }
    [major, minor, patch, rc] if major == &"0" => {
      // text = format!("{}.{}.{}a-RC{}", major, minor, patch, rc);
      (
        Some((*major).to_string()),
        Some((*minor).to_string()),
        Some((*patch).to_string()),
        Some((*rc).to_string()),
      )
    }
    _ => {
      dbg!("Failed to normalise mod's quoted game version");
      (None, None, None, None)
    }
  }
}

pub enum StarsectorVersionDiff {
  Major,
  Minor,
  Patch,
  RC,
  None,
}

impl From<(&GameVersion, &GameVersion)> for StarsectorVersionDiff {
  fn from(vals: (&GameVersion, &GameVersion)) -> Self {
    match vals {
      ((mod_major, ..), (game_major, ..)) if mod_major != game_major => {
        StarsectorVersionDiff::Major
      }
      ((_, mod_minor, ..), (_, game_minor, ..)) if mod_minor != game_minor => {
        StarsectorVersionDiff::Minor
      }
      ((.., mod_patch, _), (.., game_patch, _)) if mod_patch != game_patch => {
        StarsectorVersionDiff::Patch
      }
      ((.., mod_rc), (.., game_rc)) if mod_rc != game_rc => StarsectorVersionDiff::RC,
      _ => StarsectorVersionDiff::None,
    }
  }
}

impl From<StarsectorVersionDiff> for KeyOrValue<Color> {
  fn from(status: StarsectorVersionDiff) -> Self {
    match status {
      StarsectorVersionDiff::Major => crate::theme::RED_KEY.into(),
      StarsectorVersionDiff::Minor => crate::theme::ORANGE_KEY.into(),
      StarsectorVersionDiff::Patch => crate::theme::YELLOW_KEY.into(),
      StarsectorVersionDiff::RC => crate::theme::BLUE_KEY.into(),
      StarsectorVersionDiff::None => crate::theme::GREEN_KEY.into(),
    }
  }
}

pub fn default_true() -> bool {
  true
}

/// A bad trait
pub trait Collection<T, U> {
  fn insert(&mut self, item: T);

  fn len(&self) -> usize;

  fn is_empty(&self) -> bool {
    self.len() == 0
  }

  fn drain(&mut self) -> U;
}

impl<A: Clone + Hash + Eq, B, C> Collection<(A, B, C), Vec<(A, B, C)>> for HashMap<A, (A, B, C)> {
  fn insert(&mut self, item: (A, B, C)) {
    HashMap::insert(self, item.0.clone(), item);
  }

  fn len(&self) -> usize {
    self.len()
  }

  fn drain(&mut self) -> Vec<(A, B, C)> {
    self.drain().map(|(_, v)| v).collect()
  }
}

impl<A: Clone + Hash + Eq, B> Collection<(A, B), Vec<(A, B)>> for HashMap<A, B> {
  fn insert(&mut self, (k, v): (A, B)) {
    HashMap::insert(self, k, v);
  }

  fn len(&self) -> usize {
    self.len()
  }

  fn drain(&mut self) -> Vec<(A, B)> {
    self.drain().collect()
  }
}

impl<A: Clone + Hash + Eq, B> Collection<(A, B), HashMap<A, B>> for HashMap<A, B> {
  fn insert(&mut self, (k, v): (A, B)) {
    HashMap::insert(self, k, v);
  }

  fn len(&self) -> usize {
    self.len()
  }

  fn drain(&mut self) -> HashMap<A, B> {
    let mut drain = HashMap::new();
    std::mem::swap(self, &mut drain);
    drain
  }
}

impl Collection<Arc<ModEntry>, Vec<Arc<ModEntry>>> for Vec<Arc<ModEntry>> {
  fn insert(&mut self, item: Arc<ModEntry>) {
    self.push(item);
  }

  fn len(&self) -> usize {
    self.len()
  }

  fn drain(&mut self) -> Vec<Arc<ModEntry>> {
    self.split_off(0)
  }
}

pub struct LoadBalancer<T: Any + Send, DRAIN: Any + Send, SINK: Default + Collection<T, DRAIN>> {
  tx: std::sync::LazyLock<RwLock<Weak<mpsc::UnboundedSender<T>>>>,
  sink: PhantomData<SINK>,
  selector: Selector<DRAIN>,
}

impl<T: Any + Send, U: Any + Send, SINK: Default + Collection<T, U> + Send>
  LoadBalancer<T, U, SINK>
{
  pub const fn new(selector: Selector<U>) -> Self {
    Self {
      tx: std::sync::LazyLock::new(Default::default),
      sink: PhantomData,
      selector,
    }
  }

  pub fn sender(&self, ext_ctx: ExtEventSink) -> Arc<mpsc::UnboundedSender<T>> {
    let sender = self.tx.read().unwrap();
    if let Some(tx) = sender.upgrade() {
      tx
    } else {
      drop(sender);
      let Ok(mut sender) = self.tx.try_write() else {
        return self.sender(ext_ctx);
      };
      let (tx, mut rx) = mpsc::unbounded_channel::<T>();
      let tx = Arc::new(tx);
      let selector = self.selector;
      tokio::task::spawn(async move {
        let sleep = tokio::time::sleep(std::time::Duration::from_millis(50));
        tokio::pin!(sleep);

        let mut sink = SINK::default();
        loop {
          select! {
            message = rx.recv() => {
              if let Some(message) = message {
                sink.insert(message);
              } else {
                if !sink.is_empty() {
                  let vals = sink.drain();
                  let _ = ext_ctx.submit_command(selector, vals, Target::Auto);
                }
                break
              }
            },
            () = &mut sleep => {
              if !sink.is_empty() {
                let vals = sink.drain();
                let _ = ext_ctx.submit_command(selector, vals, Target::Auto);
              }
              sleep.as_mut().reset(tokio::time::Instant::now() + std::time::Duration::from_millis(50));
            }
          }
        }
      });

      *sender = Arc::downgrade(&tx);

      tx
    }
  }
}

#[extend::ext(name = Tap)]
pub impl<T> T {
  fn tap<U>(mut self, func: impl FnOnce(&mut Self) -> U) -> Self {
    func(&mut self);
    self
  }

  fn pipe<U>(self, func: impl FnOnce(Self) -> U) -> U
  where
    Self: Sized,
  {
    func(self)
  }
}

#[derive(Debug, Clone)]
pub struct DataTimer(TimerToken);

impl PartialEq<TimerToken> for DataTimer {
  fn eq(&self, other: &TimerToken) -> bool {
    &self.0 == other
  }
}

impl DataTimer {
  pub const INVALID: Self = Self(TimerToken::INVALID);
}

impl Data for DataTimer {
  fn same(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl Deref for DataTimer {
  type Target = TimerToken;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl From<TimerToken> for DataTimer {
  fn from(value: TimerToken) -> Self {
    Self(value)
  }
}

#[macro_export]
macro_rules! match_command {
  ($val:expr, $default:expr => {$($($selector:ident)::* $(($bind:ident))? => $body:expr),+ $(,)? }) => {
    match $val {
      val => match () {
        $(
          () if val.is($($selector)::*) => {
            let _selector = $($selector)::*;
            $(let $bind = val.get_unchecked(_selector);)?
            $body
          }
        )+
        _ => {
          $default
        }
      }
    }
  };
  ($val:expr, $default:expr => {$($($($selector:ident)::*, )+ => $body:expr),+ $(,)? }) => {
    match $val {
      val => match () {
        $(
          $(
            () if val.is($($selector)::*) => {
              $body
            }
          )+
        )+
        _ => {
          $default
        }
      }
    }
  };
}

#[extend::ext(name = PrintAndPanic)]
pub impl<T, E: Debug> Result<T, E> {
  fn inspanic(self, msg: &str) {
    self.inspect_err(|e| eprintln!("{e:?}")).expect(msg);
  }
}

pub struct ValueFormatter;

impl druid::text::Formatter<u32> for ValueFormatter {
  fn format(&self, value: &u32) -> String {
    value.to_string()
  }

  fn validate_partial_input(
    &self,
    input: &str,
    _sel: &druid::text::Selection,
  ) -> druid::text::Validation {
    match input.parse::<u32>() {
      Err(err) if !input.is_empty() => druid::text::Validation::failure(err),
      _ => druid::text::Validation::success(),
    }
  }

  fn value(&self, input: &str) -> Result<u32, druid::text::ValidationError> {
    input
      .parse::<u32>()
      .map_err(druid::text::ValidationError::new)
  }
}

// dbg macro that returns ()
#[macro_export]
macro_rules! bang {
  ($($x:tt)*) => {
    {
      dbg!($($x)*);
    }
  };
}

// print macro that only runs in debug builds
#[macro_export]
macro_rules! d_println {
  ($($arg:tt)*) => {
    {
      #[cfg(debug_assertions)]
      println!($($arg)*)
    }
  };
}

// error print that only runs in debug builds
#[macro_export]
macro_rules! d_eprintln {
  ($($arg:tt)*) => (#[cfg(debug_assertions)] eprintln!($($arg)*));
}

#[extend::ext(name = FnWidgetToMaybe)]
pub impl<T: Data, W: Widget<T> + 'static, F: Fn() -> W + 'static> F {
  fn or_maybe_empty(self) -> Maybe<T> {
    Maybe::or_empty(self)
  }
}

trait Get<Idx> {
  type Output: ?Sized;

  fn get(&self, idx: Idx) -> Option<&Self::Output>;
}

impl<T: std::ops::Index<usize>> Get<usize> for T
where
  for<'a> &'a Self: IntoIterator<IntoIter: ExactSizeIterator>,
{
  type Output = T::Output;

  fn get(&self, idx: usize) -> Option<&T::Output> {
    if idx < self.into_iter().len() {
      Some(&self[idx])
    } else {
      None
    }
  }
}

trait GetMut<Idx>: Get<Idx> {
  fn get_mut(&mut self, idx: Idx) -> Option<&mut Self::Output>;
}

impl<T: std::ops::IndexMut<usize> + Get<usize, Output = <T as std::ops::Index<usize>>::Output>>
  GetMut<usize> for T
where
  for<'a> &'a Self: IntoIterator<IntoIter: ExactSizeIterator>,
{
  fn get_mut(&mut self, idx: usize) -> Option<&mut Self::Output> {
    if idx < self.into_iter().len() {
      Some(&mut self[idx])
    } else {
      None
    }
  }
}

#[cfg(test)]
mod test {
  use crate::app::util::GetMut;

  macro_rules! get_mut_list {
    ($($e:expr),+) => {
      vec![
        $(Box::new($e) as Box<dyn GetMut<usize, Output = i32>>),+
      ]
    };
  }

  #[test]
  fn get_mut_blanket_impl() {
    let list = get_mut_list![[0, 1, 2, 3], vec![0]];

    for ele in list {
      ele.get(0);
    }
  }
}

use std::{
  any::Any,
  collections::HashMap,
  fmt::Debug,
  hash::Hash,
  io::Read,
  marker::PhantomData,
  ops::Deref,
  path::PathBuf,
  rc::Rc,
  sync::{Arc, LazyLock, RwLock, Weak},
};

use common::{controllers::HoverController, labels::LabelExt as _};
use druid::{
  lens::{Identity, InArc},
  widget::{Label, LabelText, Maybe, ScopeTransfer},
  Color, Data, Event, ExtEventSink, KeyOrValue, Lens, MouseEvent, Selector, Target, TimerToken,
  Widget, WidgetExt,
};
use json_comments::StripComments;
use regex::Regex;
use tokio::{select, sync::mpsc};
use web_client::WebClient;

use crate::app::{
  mod_entry::{GameVersion, ModEntry, ModVersionMeta},
  settings::button_painter,
};

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
  #[error("No such file")]
  NoSuchFile,
  #[error("File read error")]
  ReadError,
  #[error("File format error")]
  FormatError,
  #[error("Archive error")]
  ZipError(#[from] zip::result::ZipError),
  #[error("IO error")]
  IoError(#[from] std::io::Error),
  #[error("Serialization error")]
  SerializationError(#[from] serde_json::Error),
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

pub const MASTER_VERSION_RECEIVED: Selector<(String, Result<ModVersionMeta, anyhow::Error>)> =
  Selector::new("remote_version_received");

pub async fn get_master_version(
  client: &WebClient,
  ext_sink: Option<ExtEventSink>,
  remote_url: String,
  id: String,
) -> Option<ModVersionMeta> {
  let request = async |client: &WebClient| {
    let res = client.get(remote_url).await;

    match res {
      Err(err) => (id, Err(err.into())),
      Ok(remote) => {
        let mut stripped = String::new();
        if StripComments::new(remote.as_bytes())
          .read_to_string(&mut stripped)
          .is_ok()
          && let Ok(normalized) = handwritten_json::normalize(&stripped)
          && let Ok(remote) = json5::from_str::<ModVersionMeta>(&normalized)
        {
          (id, Ok(remote))
        } else {
          (id, Err(anyhow::anyhow!("Parse error. Payload:\n{remote}")))
        }
      }
    }
  };

  if let Some(ext_sink) = ext_sink {
    let client = client.clone();
    tokio::spawn(async move {
      let payload = request(&client).await;

      if let Err(err) = ext_sink.submit_command(MASTER_VERSION_RECEIVED, payload, Target::Auto) {
        eprintln!("Failed to submit remote version data {err}");
      }
    });
    None
  } else {
    request(client).await.1.ok()
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

    let mut version_class = zip
      .by_name("com/fs/starfarer/Version.class")
      .map_err(|_| LoadError::NoSuchFile)?;

    let mut buf: Vec<u8> = Vec::new();
    version_class
      .read_to_end(&mut buf)
      .map_err(|_| LoadError::ReadError)
      .and_then(|_| {
        class_parser(&buf)
          .map_err(|_| LoadError::FormatError)
          .map(|(_, class_file)| class_file)
      })
      .and_then(|class_file| {
        class_file
          .fields
          .iter()
          .find_map(|f| {
            if let classfile_parser::constant_info::ConstantInfo::Utf8(name) =
              &class_file.const_pool[(f.name_index - 1) as usize]
              && name.utf8_string == "versionOnly"
              && let Ok((_, attr)) =
                classfile_parser::attribute_info::constant_value_attribute_parser(
                  &f.attributes.first().unwrap().info,
                )
              && let classfile_parser::constant_info::ConstantInfo::Utf8(utf_const) =
                &class_file.const_pool[attr.constant_value_index as usize]
            {
              Some(utf_const.utf8_string.clone())
            } else {
              None
            }
          })
          .ok_or(LoadError::FormatError)
      })
  })
  .await
  .map_err(|_| LoadError::ReadError)
  .flatten();

  if res.is_err() {
    static RE: LazyLock<Regex> =
      LazyLock::new(|| Regex::new(r"Starting Starsector (.*) launcher").unwrap());

    res = fs::read(install_dir.join("starsector-core").join("starsector.log"))
      .await
      .map_err(|_| LoadError::ReadError)
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

#[must_use]
pub fn default_true() -> bool {
  true
}

pub struct Button2;

impl Button2 {
  pub fn new<T: Data, W: Widget<T> + 'static>(label: W) -> impl Widget<T> {
    label
      .padding((8., 4.))
      .background(button_painter())
      .controller(HoverController::default())
  }

  pub fn from_label<T: Data>(label: impl Into<LabelText<T>>) -> impl Widget<T> {
    Self::new(Label::wrapped_into(label).with_text_size(18.))
  }
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

#[must_use]
pub fn option_ptr_cmp<T>(this: &Option<Rc<T>>, other: &Option<Rc<T>>) -> bool {
  if let Some(this) = this
    && let Some(other) = other
  {
    Rc::ptr_eq(this, other)
  } else {
    false
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

pub fn ident_arc<T: Data>() -> InArc<Identity> {
  InArc::new::<T, T>(Identity)
}

#[must_use]
pub fn ident_rc<T: Data>() -> InRc<Identity> {
  InRc::new::<T, T>(Identity)
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

pub trait TransferRead<State, In> = Fn(&mut State, &In);
pub trait TransferWrite<State, In> = Fn(&State, &mut In);

pub struct FnTransfer<
  In: Data,
  State: Data,
  R: TransferRead<State, In>,
  W: TransferWrite<State, In>,
> {
  read: R,
  write: W,
  _read: PhantomData<In>,
  _write: PhantomData<State>,
}

impl<In: Data, State: Data, R: TransferRead<State, In>, W: TransferWrite<State, In>>
  FnTransfer<In, State, R, W>
{
  pub fn new(read: R, write: W) -> Self {
    Self {
      read,
      write,
      _read: PhantomData,
      _write: PhantomData,
    }
  }
}

impl<In: Data, State: Data, R: TransferRead<State, In>, W: TransferWrite<State, In>> ScopeTransfer
  for FnTransfer<In, State, R, W>
{
  type In = In;
  type State = State;

  fn read_input(&self, state: &mut Self::State, input: &Self::In) {
    (self.read)(state, input);
  }

  fn write_back_input(&self, state: &Self::State, input: &mut Self::In) {
    (self.write)(state, input);
  }
}

// TODO: macro that syncs fields with same names between two structs using
// existing lens impls on tuples of lenses

pub struct PartialScopeTransfer<In, State> {
  read: Box<dyn TransferRead<State, In>>,
  write: Box<dyn TransferWrite<State, In>>,
}

impl<In, State> PartialScopeTransfer<In, State> {
  pub fn new<Prt: Data>(
    lens_state: impl Lens<State, Prt> + Clone + 'static,
    lens_in: impl Lens<In, Prt> + Clone + 'static,
  ) -> PartialScopeTransfer<In, State> {
    PartialScopeTransfer {
      read: {
        let lens_state = lens_state.clone();
        let lens_in = lens_in.clone();
        Box::new(move |state: &mut State, data: &In| {
          let partial = lens_in.with(data, std::clone::Clone::clone);
          lens_state.with_mut(state, |inner| {
            if !inner.same(&partial) {
              *inner = partial;
            }
          });
        })
      },
      write: Box::new(move |state, data| {
        let partial = lens_state.with(state, std::clone::Clone::clone);
        lens_in.with_mut(data, |inner| {
          if !inner.same(&partial) {
            *inner = partial;
          }
        });
      }),
    }
  }
}

impl<In: Data, State: Data> ScopeTransfer for PartialScopeTransfer<In, State> {
  type In = In;
  type State = State;

  fn read_input(&self, state: &mut State, data: &In) {
    (self.read)(state, data);
  }

  fn write_back_input(&self, state: &State, data: &mut In) {
    (self.write)(state, data);
  }
}

#[derive(Clone)]
pub struct Convert<T, U> {
  outer: PhantomData<T>,
  inner: PhantomData<U>,
}

impl<T, U> Default for Convert<T, U> {
  fn default() -> Self {
    Self {
      outer: PhantomData,
      inner: PhantomData,
    }
  }
}

impl<T, U> Convert<T, U> {
  pub fn new() -> Self {
    Self::default()
  }
}

impl<T: From<U> + Clone, U: Data + From<T>> Lens<T, U> for Convert<T, U> {
  fn with<V, F: FnOnce(&U) -> V>(&self, data: &T, f: F) -> V {
    let data = data.clone().into();
    f(&data)
  }

  fn with_mut<V, F: FnOnce(&mut U) -> V>(&self, data: &mut T, f: F) -> V {
    let mut val = data.clone().into();
    let res = f(&mut val);
    *data = val.into();

    res
  }
}

#[extend::ext(name = EventExt)]
pub impl Event {
  fn get_cmd<T: 'static>(&self, selector: Selector<T>) -> Option<&T> {
    if let Event::Command(cmd) = self {
      cmd.get(selector)
    } else {
      None
    }
  }

  fn is_cmd<T: 'static>(&self, selector: Selector<T>) -> bool {
    if let Event::Command(cmd) = self {
      cmd.is(selector)
    } else {
      false
    }
  }

  fn as_mouse_up(&self) -> Option<&MouseEvent> {
    if let Event::MouseUp(mouse) = self {
      Some(mouse)
    } else {
      None
    }
  }
}

/// A `Lens` that exposes data within an `Arc` with copy-on-write semantics
///
/// A copy is only made in the event that a different value is written.
#[derive(Debug, Copy, Clone)]
pub struct InRc<L> {
  inner: L,
}

impl<L> InRc<L> {
  /// Adapt a lens to operate on an `Arc`
  ///
  /// See also `LensExt::in_arc`
  pub fn new<A, B>(inner: L) -> Self
  where
    A: Clone,
    B: Data,
    L: Lens<A, B>,
  {
    Self { inner }
  }
}

impl<A, B, L> Lens<Rc<A>, B> for InRc<L>
where
  A: Clone,
  B: Data,
  L: Lens<A, B>,
{
  fn with<V, F: FnOnce(&B) -> V>(&self, data: &Rc<A>, f: F) -> V {
    self.inner.with(data, f)
  }

  fn with_mut<V, F: FnOnce(&mut B) -> V>(&self, data: &mut Rc<A>, f: F) -> V {
    let mut temp = self.inner.with(data, std::clone::Clone::clone);
    let v = f(&mut temp);
    if self.inner.with(data, |x| !x.same(&temp)) {
      self.inner.with_mut(Rc::make_mut(data), |x| *x = temp);
    }
    v
  }
}

pub trait IsSendSync: Send + Sync {}

#[extend::ext(name = FnWidgetToMaybe)]
pub impl<T: Data, W: Widget<T> + 'static, F: Fn() -> W + 'static> F {
  fn or_maybe_empty(self) -> Maybe<T> {
    Maybe::or_empty(self)
  }
}

use std::{
  collections::VecDeque,
  io::Cursor,
  path::{Path, PathBuf},
  sync::LazyLock,
};

use anyhow::Context;
use compress_tools::uncompress_archive;
use consts::MIKO_JDK_VER;
use druid::{
  im::Vector,
  text::RichTextBuilder,
  widget::{Either, Flex, Label, Radio, RawLabel, Spinner},
  Data, Lens, Selector, Widget, WidgetExt, WidgetId,
};
use druid_widget_nursery::{
  table::{FlexTable, TableRow},
  WidgetExt as _,
};
use flate2::read::GzDecoder;
use rand::random;
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::Display;
use tar::Archive;
use tempfile::TempDir;
use tokio::runtime::Handle;
use webview_shared::ExtEventSinkExt;

use super::{tool_card, vmparams::VMParams};
use crate::{
  app::{
    controllers::AnimController,
    mod_entry::GameVersion,
    overlays::Popup,
    util::{h2_fixed, parse_game_version, ShadeColor, WidgetExtEx},
    SharedFromEnv,
  },
  bang, d_println, theme,
  widgets::card::Card,
};

#[derive(Debug, Clone, Data, Lens)]
pub struct Swapper {
  #[data(eq)]
  pub current_flavour: Flavour,
  pub cached_flavours: Vector<Flavour>,
  #[data(eq)]
  pub install_dir: PathBuf,
  pub jre_23: bool,
}

impl Swapper {
  pub fn view() -> impl Widget<Self> {
    const LINK_CLICKED: Selector = Selector::new("swapper.very_old.link");

    let mut inactive_text = RichTextBuilder::new();
    inactive_text.push(
      "\
      Starsector uses Java 7 by default, which is ",
    );
    inactive_text
      .push("very old.")
      .link(LINK_CLICKED)
      .underline(true);
    inactive_text.push(
      " Replacing it with a newer version usually improves long term performance, memory usage \
       and reliability.",
    );
    let inactive_text = inactive_text.build();
    let mut active_text = RichTextBuilder::new();
    active_text.push(
      "\
      Starsector uses Java 7 by default, which ",
    );
    active_text
      .push("came out in 2011!")
      .link(LINK_CLICKED)
      .underline(true);
    active_text.push(
      " Replacing it with a newer version usually improves long term performance, memory usage \
       and reliability.",
    );
    let active_text = active_text.build();

    tool_card().build(
      Flex::column()
        .cross_axis_alignment(druid::widget::CrossAxisAlignment::Start)
        .with_child(h2_fixed("Java Runtime Swapper"))
        .with_child(
          RawLabel::new()
            .with_line_break_mode(druid::widget::LineBreaking::WordWrap)
            .scope_with::<bool, _, _>(|_| false, {
              let inactive_text = inactive_text.clone();
              move |widget| {
                widget.on_command(LINK_CLICKED, move |_, _, state| {
                  state.inner = !state.inner;
                  state.outer = if state.inner {
                    active_text.clone()
                  } else {
                    inactive_text.clone()
                  }
                })
              }
            })
            .scope_independent(move || inactive_text.clone()),
        )
        .with_default_spacer()
        .with_child(
          Flex::row()
            .with_flex_child(
              FlexTable::new()
                .with_row(Self::swapper_row(Flavour::Original, "Original (Java 7)"))
                .with_row(Self::swapper_row(
                  Flavour::Miko,
                  "Java 23 by Mikohime (New!)",
                ))
                .with_row(Self::swapper_row(Flavour::Wisp, "Archived Java 8"))
                .with_row(Self::swapper_row(Flavour::Azul, "Azul Java 8")),
              2.,
            )
            .with_flex_spacer(1.)
            .env_scope(|env, _| env.set(druid::theme::CURSOR_COLOR, druid::Color::BLACK.lighter())),
        )
        .padding((Card::CARD_INSET, 0.0)),
    )
  }

  fn swapper_row(flavour: Flavour, label: &str) -> TableRow<Self> {
    const DOWNLOAD_FAILED: Selector<Flavour> = Selector::new("jre.download.failed");

    let empty_if_not = move |_: &Swapper, env: &druid::Env| {
      static _0_97_A_RC_11: LazyLock<GameVersion> =
        LazyLock::new(|| parse_game_version("0.97a-RC11"));

      let game_version = &env.shared_data().game_version;

      !flavour.is_miko() || Some(&*_0_97_A_RC_11) == game_version.as_ref()
    };

    TableRow::new()
      .with_child(
        Radio::new(label, flavour)
          .lens(Swapper::current_flavour)
          .on_change(move |ctx, old, data, _| {
            let old_flavour = old.current_flavour;
            if flavour.is_miko() {
              if !old_flavour.is_miko() {
                data.jre_23 = true;
                ctx.submit_command(VMParams::SAVE_VMPARAMS);
                return;
              }
            } else {
              data.jre_23 = false
            }

            ctx.submit_command(VMParams::SAVE_VMPARAMS);
            ctx.submit_command(Popup::OPEN_POPUP.with(Popup::custom(Self::swap_in_progress_popup)));
            let install_dir = data.install_dir.clone();
            let ext_ctx = ctx.get_external_handle();
            tokio::spawn(async move {
              let res = match flavour {
                Flavour::Original => revert_jre(install_dir).await,
                _ => flavour.swap_jre(install_dir).await,
              };

              let _ = ext_ctx.submit_command_global(Popup::DISMISS, ());
              if res.is_err() {
                // let _ = ext_ctx.submit_command_global(DOWNLOAD_FAILED,
                // flavour);
              }
            });
          })
          .disabled_if(move |data, _| {
            !(flavour == Flavour::Original
              || data.current_flavour == flavour
              || data.cached_flavours.contains(&flavour))
          })
          .empty_if_not(empty_if_not),
      )
      .with_child({
        let id = WidgetId::next();
        let builder = Card::builder()
          .with_insets((0., 10.))
          .with_corner_radius(2.)
          .with_shadow_length(1.)
          .with_shadow_increase(1.);
        let button = Either::new(
          move |data: &Swapper, _: &druid::Env| {
            data.current_flavour == flavour || data.cached_flavours.contains(&flavour)
          },
          builder
            .clone()
            .with_background(druid::theme::BUTTON_DARK)
            .hoverable(|_| {
              Label::new("Downloaded")
                .env_scope(|env, _| {
                  env.set(
                    druid::theme::TEXT_SIZE_NORMAL,
                    env.get(druid::theme::TEXT_SIZE_NORMAL) * 0.6,
                  );
                  env.set(
                    druid::theme::DISABLED_TEXT_COLOR,
                    env.get(druid::theme::TEXT_COLOR),
                  );
                })
                .align_horizontal(druid::UnitPoint::CENTER)
                .expand_width()
                .padding((0., -2.5))
            }),
          Either::new(
            |data, _: &druid::Env| *data,
            builder
              .clone()
              .with_background(theme::GREEN_KEY)
              .hoverable(|_| {
                Label::new("Download")
                  .with_text_color(theme::ON_GREEN_KEY)
                  .env_scope(|env, _| {
                    env.set(
                      druid::theme::TEXT_SIZE_NORMAL,
                      env.get(druid::theme::TEXT_SIZE_NORMAL) * 0.6,
                    )
                  })
                  .align_horizontal(druid::UnitPoint::CENTER)
                  .expand_width()
                  .padding((0., -2.5))
              }),
            builder
              .with_background(theme::GREEN_KEY)
              .hoverable(move |_| {
                Label::dynamic(|data: &f64, _| {
                  format!("Downloading{}", ".".repeat(data.floor() as usize))
                })
                .controller(
                  AnimController::new(
                    0.,
                    4.,
                    druid_widget_nursery::animation::AnimationCurve::LINEAR,
                  )
                  .with_transform(|v| v.floor())
                  .with_duration(2.5)
                  .looping(),
                )
                .with_id(id)
                .on_command(DOWNLOAD_STARTED, move |ctx, payload, _| {
                  if *payload == flavour {
                    ctx.submit_command(AnimController::<f64>::ANIM_START.to(id));
                  }
                })
                .scope_independent(|| 0.)
                .env_scope(|env, _| {
                  env.set(
                    druid::theme::TEXT_SIZE_NORMAL,
                    env.get(druid::theme::TEXT_SIZE_NORMAL) * 0.6,
                  );
                  env.set(
                    druid::theme::DISABLED_TEXT_COLOR,
                    env.get(theme::ON_GREEN_KEY),
                  );
                })
                .align_horizontal(druid::UnitPoint::CENTER)
                .expand_width()
                .padding((0., -2.5))
              })
              .disabled(),
          )
          .on_command(DOWNLOAD_STARTED, move |_, payload, data| {
            if *payload == flavour {
              *data = false;
            }
          })
          .on_command(DOWNLOAD_FAILED, move |_, payload, data| {
            if *payload == flavour {
              *data = true;
            }
          })
          .on_command(DOWNLOAD_COMPLETE, move |_, payload, data| {
            if *payload == flavour {
              *data = true;
            }
          })
          .scope_independent(|| true),
        )
        .fix_width(175.);

        const DOWNLOAD_COMPLETE: Selector<Flavour> = Selector::new("jre.download.complete");
        const DOWNLOAD_STARTED: Selector<Flavour> = Selector::new("jre.download.start");
        match flavour {
          Flavour::Miko if cfg!(target_os = "macos") => button.invisible().disabled().boxed(),
          Flavour::Original => button.invisible().disabled().boxed(),
          _ => button
            .on_click(move |ctx, data, _| {
              let install_dir = data.install_dir.clone();
              let ext_ctx = ctx.get_external_handle();
              tokio::spawn(async move {
                let res = flavour.download(install_dir).await;
                match res {
                  Ok(_) => {
                    if let Err(err) = ext_ctx.submit_command_global(DOWNLOAD_COMPLETE, flavour) {
                      dbg!(err);
                    }
                  }
                  Err(err) => {
                    dbg!(err);
                  }
                }
              });
              ctx.set_disabled(true);
              ctx.submit_command(DOWNLOAD_STARTED.with(flavour))
            })
            .on_command(DOWNLOAD_COMPLETE, move |_, payload, data| {
              if *payload == flavour {
                data.cached_flavours.push_back(flavour)
              }
            })
            .disabled_if(move |data: &Swapper, _| {
              data.current_flavour == flavour || data.cached_flavours.contains(&flavour)
            })
            .boxed(),
        }
        .empty_if_not(empty_if_not)
      })
  }

  pub async fn get_cached_jres(install_dir: PathBuf) -> (Flavour, Vec<Flavour>) {
    let mut available: Vec<Flavour> = Flavour::iter()
      .filter_map(|f| {
        let sub_path = if f == Flavour::Original {
          ORIGINAL_JRE_BACKUP.to_owned()
        } else {
          format!("jre_{}", f)
        };
        let path = install_dir.join(sub_path);

        path.exists().then_some(f)
      })
      .collect();

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    if install_dir.join("Miko_R3.txt").exists() && install_dir.join(MIKO_JDK_VER).exists() {
      available.push(Flavour::Miko)
    }

    let current = Swapper::get_actual_jre(&install_dir);

    available.push(current);

    (current, available)
  }

  fn swap_in_progress_popup() -> Box<dyn Widget<()>> {
    Spinner::new().fix_size(50., 50.).boxed()
  }

  fn get_actual_jre(install_dir: &Path) -> Flavour {
    fn inner(install_dir: &Path) -> anyhow::Result<Flavour> {
      let current_jre = install_dir.join(consts::JRE_PATH);

      let val: anyhow::Result<Flavour> = std::fs::read_to_string(current_jre.join(".moss"))
        .map_err(anyhow::Error::new)
        .and_then(|s| serde_json::from_str(&s).map_err(anyhow::Error::new));

      if let Err(err) = val {
        std::fs::read_to_string(current_jre.join("release"))
          .is_ok_and(|release| {
            release
              .split_ascii_whitespace()
              .next()
              .is_some_and(|version| version.eq_ignore_ascii_case(r#"JAVA_VERSION="1.7.0""#))
          })
          .then_some(Flavour::Original)
          .ok_or(anyhow::anyhow!(
            "Could not parse release file in (assumed) Java 7 folder"
          ))
          .context(err)
      } else {
        val
      }
    }

    inner(install_dir)
      .inspect_err(|e| bang!(e))
      .unwrap_or(Flavour::Original)
  }
}

#[derive(
  Debug,
  Copy,
  Clone,
  Display,
  Data,
  Serialize,
  Deserialize,
  PartialEq,
  Eq,
  strum_macros::EnumIter,
  strum_macros::FromRepr,
)]
pub enum Flavour {
  Miko,
  Coretto,
  Hotspot,
  Wisp,
  Azul,
  Original,
}

const ORIGINAL_JRE_BACKUP: &str = "jre7";
const JRE_BACKUP: &str = "jre.bak";

impl Flavour {
  async fn download(self, install_dir: PathBuf) -> Result<(), anyhow::Error> {
    let cached_jre = install_dir.join(match self {
      #[cfg(any(target_os = "linux", target_os = "windows"))]
      Flavour::Miko => MIKO_JDK_VER.to_owned(),
      _ => format!("jre_{}", self),
    });

    if !cached_jre.exists() {
      let tempdir = self.unpack(&install_dir).await?;

      let search_stratgey = self.get_search_strategy();
      let jre_8 = Self::find_jre(tempdir.path(), search_stratgey).await?;

      serde_json::to_writer_pretty(
        std::fs::OpenOptions::new()
          .create(true)
          .write(true)
          .open(jre_8.join(".moss"))?,
        &self,
      )?;

      std::fs::rename(jre_8, &cached_jre)?;
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    if self.is_miko() && !install_dir.join("mikohime").exists() {
      let miko_dir = Flavour::get_miko_kit(&install_dir).await?;

      Flavour::move_miko_kit(miko_dir.path()).await?;
    }

    Ok(())
  }

  async fn swap_jre(self, root: PathBuf) -> anyhow::Result<bool> {
    let cached_jre = root.join(format!("jre_{}", self));
    let stock_jre = root.join(consts::JRE_PATH);

    if Swapper::get_actual_jre(&root) == self {
      d_println!("Installed is already target flavour - no-op");
      return Ok(true);
    }

    let tempdir: TempDir;
    let jre_8 = if !cached_jre.exists() {
      tempdir = self.unpack(&root).await?;

      let search_stratgey = self.get_search_strategy();
      let jre_8 = Self::find_jre(tempdir.path(), search_stratgey).await?;

      serde_json::to_writer_pretty(
        std::fs::OpenOptions::new()
          .create(true)
          .write(true)
          .open(jre_8.join(".moss"))?,
        &self,
      )?;

      std::fs::rename(jre_8, &cached_jre)?;

      cached_jre
    } else {
      cached_jre
    };

    if stock_jre.exists() {
      std::fs::rename(&stock_jre, get_backup_path(&stock_jre)?)?;
    }
    std::fs::rename(jre_8, &stock_jre)?;

    Ok(false)
  }

  fn as_const(&self) -> (&'static str, FindBy) {
    match self {
      Flavour::Coretto => consts::CORETTO,
      Flavour::Hotspot => consts::HOTSPOT,
      Flavour::Wisp => consts::WISP,
      Flavour::Azul => consts::AZUL,
      Flavour::Miko => consts::MIKO_JDK,
      Flavour::Original => unimplemented!(),
    }
  }

  fn get_url(&self) -> &'static str {
    self.as_const().0
  }

  fn get_search_strategy(&self) -> FindBy {
    self.as_const().1
  }

  async fn unpack(&self, root: &Path) -> anyhow::Result<TempDir> {
    let url = Self::get_url(self);

    let tempdir = TempDir::new_in(root).context("Create tempdir")?;

    let mut res = reqwest::get(url).await?;

    let mut buf = Vec::new();
    while let Some(bytes) = res.chunk().await? {
      buf.append(&mut bytes.to_vec())
    }

    let path = root.join(tempdir.path());
    Handle::current()
      .spawn_blocking(move || -> anyhow::Result<()> {
        if infer::archive::is_gz(&buf) {
          let tar = GzDecoder::new(Cursor::new(buf));
          let mut archive = Archive::new(tar);
          archive.unpack(&path).context("Unpack tarball")
        } else if infer::archive::is_zip(&buf) {
          let mut zip = zip::ZipArchive::new(Cursor::new(buf))?;
          zip.extract(&path).context("Unpack zip")
        } else {
          uncompress_archive(Cursor::new(buf), &path, compress_tools::Ownership::Ignore)
            .context("Failed to unpack")
        }
      })
      .await??;

    Ok(tempdir)
  }

  async fn find_jre(root: &Path, search_strategy: FindBy) -> anyhow::Result<PathBuf> {
    let mut visit = VecDeque::new();
    visit.push_back(root.to_path_buf());
    Handle::current()
      .spawn_blocking(move || {
        while let Some(path) = visit.pop_front() {
          if let Ok(mut iter) = path.read_dir() {
            while let Some(Ok(file)) = iter.next() {
              if let Ok(file_type) = file.file_type() {
                if file_type.is_dir() {
                  if search_strategy == FindBy::Bin && file.file_name().eq_ignore_ascii_case("bin")
                  {
                    return Some(path);
                  } else if search_strategy == FindBy::Jre
                    && file.file_name().eq_ignore_ascii_case("jre")
                  {
                    return Some(file.path());
                  } else {
                    visit.push_back(file.path())
                  }
                }
              }
            }
          }
        }

        None
      })
      .await?
      .with_context(|| "Could not find JRE in given folder")
  }

  #[cfg(any(target_os = "linux", target_os = "windows"))]
  async fn get_miko_kit(root: &Path) -> anyhow::Result<TempDir> {
    let url = consts::MIKO_KIT;

    let tempdir = TempDir::new_in(root).context("Create tempdir")?;

    let mut res = reqwest::get(url).await?;

    let mut buf = Vec::new();
    while let Some(bytes) = res.chunk().await? {
      buf.append(&mut bytes.to_vec())
    }

    let path = root.join(tempdir.path());
    let mut zip = zip::ZipArchive::new(Cursor::new(buf))?;

    Handle::current()
      .spawn_blocking(move || zip.extract(&path).context("Unpack zip"))
      .await??;

    Ok(tempdir)
  }

  #[cfg(any(target_os = "linux", target_os = "windows"))]
  async fn move_miko_kit(miko_download: &Path) -> anyhow::Result<()> {
    let root_dir = miko_download
      .parent()
      .ok_or(anyhow::anyhow!("Parent should not be missing"))?
      .to_owned();

    let r3 = miko_download
      .join("1. Pick VMParam Size Here")
      .join("6GB (Discord Choice)")
      .join("Miko_R3.txt");

    std::fs::rename(r3, &root_dir.join("Miko_R3.txt"))?;

    // I hate this
    let install_files = miko_download.join("0. Files to put into starsector");

    fn recursive_move(from: &Path, to: &Path) -> anyhow::Result<()> {
      for entry in from.read_dir()? {
        let entry = entry?;
        let target = to.join(entry.file_name());

        if !target.exists() || (!target.is_dir() && entry.path().is_dir()) {
          std::fs::rename(entry.path(), &target)?
        } else if entry.path().is_dir() {
          recursive_move(&entry.path(), &target)?
        }
      }

      Ok(())
    }

    Handle::current()
      .spawn_blocking(move || recursive_move(&install_files, &root_dir))
      .await??;

    Ok(())
  }

  fn is_miko(&self) -> bool {
    matches!(self, Flavour::Miko)
  }
}

fn get_backup_path(stock_jre: &Path) -> Result<PathBuf, anyhow::Error> {
  let is_original = std::fs::read_to_string(stock_jre.join("release")).is_ok_and(|release| {
    release
      .split_ascii_whitespace()
      .next()
      .is_some_and(|version| version.eq_ignore_ascii_case(r#"JAVA_VERSION="1.7.0""#))
  });

  let mut backup = stock_jre.with_file_name(if is_original {
    ORIGINAL_JRE_BACKUP.to_string()
  } else if stock_jre.join(".moss").exists() {
    let flavour: Flavour =
      serde_json::from_str(&std::fs::read_to_string(stock_jre.join(".moss"))?)?;
    format!("jre_{}", flavour)
  } else {
    JRE_BACKUP.to_string()
  });
  while backup.exists() {
    backup.set_extension(random::<u16>().to_string());
  }

  Ok(backup)
}

async fn revert_jre(root: PathBuf) -> anyhow::Result<bool> {
  let current_jre = root.join(consts::JRE_PATH);
  let original_backup = current_jre.with_file_name(ORIGINAL_JRE_BACKUP);

  if Swapper::get_actual_jre(&root) == Flavour::Original {
    d_println!("Installed is already vanilla - no-op");
    return Ok(true);
  }

  if original_backup.exists() {
    if current_jre.exists() {
      if !std::fs::symlink_metadata(&current_jre)?.is_symlink() {
        std::fs::rename(&current_jre, get_backup_path(&current_jre)?)?;
      } else {
        #[cfg(target_os = "windows")]
        std::fs::remove_dir(&current_jre)?;
        #[cfg(target_family = "unix")]
        std::fs::remove_file(&current_jre)?;
      }
    }

    std::fs::rename(original_backup, &current_jre)?;

    Ok(true)
  } else {
    Ok(false)
  }
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq)]
pub enum FindBy {
  Bin,
  Jre,
}

#[cfg(target_os = "windows")]
mod consts {
  use super::FindBy;

  pub const CORETTO: (&str, FindBy) = ("https://corretto.aws/downloads/resources/8.272.10.3/amazon-corretto-8.272.10.3-windows-x64-jre.zip", FindBy::Bin);
  pub const HOTSPOT: (&str, FindBy) = ("https://github.com/AdoptOpenJDK/openjdk8-binaries/releases/download/jdk8u272-b10/OpenJDK8U-jre_x64_windows_hotspot_8u272b10.zip", FindBy::Bin);
  pub const WISP: (&str, FindBy) = (
    "https://github.com/wispborne/JRE/releases/download/jre8-271/jre8-271-Windows.zip",
    FindBy::Bin,
  );
  pub const AZUL: (&str, FindBy) = (
    "https://cdn.azul.com/zulu/bin/zulu8.68.0.21-ca-jre8.0.362-win_x64.zip",
    FindBy::Bin,
  );

  pub const MIKO_JDK: (&str, FindBy) = ("https://github.com/adoptium/temurin23-binaries/releases/download/jdk-23%2B7-ea-beta/OpenJDK-jdk_x64_windows_hotspot_ea_23-0-7.zip", FindBy::Bin);
  pub const MIKO_JDK_VER: &str = "jdk-23+7";
  pub const MIKO_KIT: &str = "https://github.com/Yumeris/Mikohime_Repo/releases/download/26.4d/Mikohime_23_R26.4f_097a-RC11_win.zip";

  pub const JRE_PATH: &str = "jre";
}
#[cfg(target_os = "linux")]
mod consts {
  use super::FindBy;

  pub const CORETTO: (&str, FindBy) = ("https://corretto.aws/downloads/resources/8.272.10.3/amazon-corretto-8.272.10.3-linux-x64.tar.gz", FindBy::Jre);
  pub const HOTSPOT: (&str, FindBy) = ("https://github.com/AdoptOpenJDK/openjdk8-binaries/releases/download/jdk8u272-b10/OpenJDK8U-jre_x64_linux_hotspot_8u272b10.tar.gz", FindBy::Bin);
  pub const WISP: (&str, FindBy) = (
    "https://github.com/wispborne/JRE/releases/download/jre8-271/jre8-271-Linux-x64.tar.gz",
    FindBy::Bin,
  );
  pub const AZUL: (&str, FindBy) = (
    "https://cdn.azul.com/zulu/bin/zulu8.68.0.21-ca-jre8.0.362-linux_x64.zip",
    FindBy::Bin,
  );

  pub const MIKO_JDK: (&str, FindBy) = ("https://github.com/adoptium/temurin23-binaries/releases/download/jdk-23%2B9-ea-beta/OpenJDK-jdk_x64_linux_hotspot_23_9-ea.tar.gz", FindBy::Bin);
  pub const MIKO_JDK_VER: &str = "jdk-23+9";
  pub const MIKO_KIT: &str = "https://github.com/Yumeris/Mikohime_Repo/releases/download/26.4d/Kitsunebi_23_R26.4f_097a-RC11_linux.zip";

  pub const JRE_PATH: &str = "jre_linux";
}
#[cfg(target_os = "macos")]
mod consts {
  use super::FindBy;

  pub const CORETTO: (&str, FindBy) = ("https://corretto.aws/downloads/resources/8.272.10.3/amazon-corretto-8.272.10.3-macosx-x64.tar.gz", FindBy::Jre);
  pub const HOTSPOT: (&str, FindBy) = ("https://github.com/AdoptOpenJDK/openjdk8-binaries/releases/download/jdk8u272-b10/OpenJDK8U-jre_x64_mac_hotspot_8u272b10.tar.gz", FindBy::Bin);
  pub const WISP: (&str, FindBy) = (
    "https://github.com/wispborne/JRE/releases/download/jre8-271/jre8-271-MacOS.zip",
    FindBy::Bin,
  );
  pub const AZUL: (&str, FindBy) = (
    "https://cdn.azul.com/zulu/bin/zulu8.68.0.21-ca-jre8.0.362-macosx_x64.zip",
    FindBy::Bin,
  );
  pub const MIKO_JDK: (&str, FindBy) = ("0.0.0.0", FindBy::Bin);
  pub const MIKO_JDK_VER: &str = "";
  pub const MIKO_KIT: &str = "";

  pub const JRE_PATH: &str = "Contents/Home";
}

#[cfg(test)]
mod test {
  use tempfile::TempDir;

  use super::{consts, revert_jre, Flavour, JRE_BACKUP, ORIGINAL_JRE_BACKUP};

  fn base_test(
    flavour: Flavour,
    mock_original: impl Into<Option<bool>>,
    test_dir: impl Into<Option<TempDir>>,
    project_test_dir: Option<TempDir>,
    expected: bool,
  ) -> (TempDir, TempDir) {
    let mock_original: Option<bool> = mock_original.into();
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .expect("Build runtime");

    runtime.block_on(async {
      let test_dir = test_dir
        .into()
        .unwrap_or_else(|| TempDir::new().expect("Create tempdir"));

      let project_test_dir =
        project_test_dir.unwrap_or_else(|| TempDir::new().expect("Create project test dir"));

      let target_path = test_dir.path().join(consts::JRE_PATH);
      if let Some(mock_original) = mock_original {
        std::fs::create_dir_all(&target_path).expect("Create mock JRE folder");

        if mock_original {
          std::fs::write(target_path.join("release"), r#"JAVA_VERSION="1.7.0""#)
            .expect("Write test release");
        }
      } else if cfg!(target_os = "macos") {
        let parent = target_path.parent().expect("Get path parent");
        std::fs::create_dir_all(parent).expect("Create parent folder");
      }

      let res = flavour
        .swap_jre(test_dir.path().to_path_buf())
        .await
        .expect("Swap JRE");

      assert_eq!(res, expected);

      if let Some(mock_original) = mock_original {
        if mock_original {
          assert!(target_path.with_file_name(ORIGINAL_JRE_BACKUP).exists());
        } else {
          assert!(target_path.with_file_name(JRE_BACKUP).exists());
        }
      }

      assert!(target_path.exists());

      assert!(target_path.join("bin").exists());

      #[cfg(target_os = "windows")]
      assert!(target_path.join("bin/java.exe").exists());
      #[cfg(not(target_os = "windows"))]
      assert!(target_path.join("bin/java").exists());

      (test_dir, project_test_dir)
    })
  }

  #[test]
  fn coretto() {
    base_test(Flavour::Coretto, true, None, None, false);
  }

  #[test]
  fn hotspot() {
    base_test(Flavour::Hotspot, true, None, None, false);
  }

  #[test]
  fn wisp() {
    base_test(Flavour::Wisp, true, None, None, false);
  }

  #[test]
  fn azul() {
    base_test(Flavour::Azul, true, None, None, false);
  }

  #[test]
  fn installs_even_if_actual_is_missing_and_unmanaged() {
    base_test(Flavour::Coretto, None, None, None, false);
  }

  // #[test]
  // fn installs_even_if_actual_is_missing_and_managed() {
  //   base_test(Flavour::Coretto, None, None, None, true, false);
  // }

  #[test]
  fn does_not_revert_when_no_original() {
    let (test_dir, _) = base_test(Flavour::Coretto, false, None, None, false);

    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .expect("Build runtime");

    let res = runtime
      .block_on(revert_jre(test_dir.path().to_owned()))
      .unwrap();

    assert!(!res);
    assert!(test_dir
      .path()
      .join(consts::JRE_PATH)
      .with_file_name(JRE_BACKUP)
      .exists());
    assert!(test_dir.path().join(consts::JRE_PATH).exists());
  }

  #[test]
  fn revert_when_original_present_and_unmanaged() {
    let (test_dir, _) = base_test(Flavour::Coretto, true, None, None, false);

    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .expect("Build runtime");

    let res = runtime
      .block_on(revert_jre(test_dir.path().to_owned()))
      .unwrap();

    assert!(res);
    assert!(test_dir
      .path()
      .join(consts::JRE_PATH)
      .with_file_name(format!("jre_{}", Flavour::Coretto))
      .exists());
    assert!(test_dir.path().join(consts::JRE_PATH).exists());
  }

  // #[test]
  // fn revert_when_original_present_and_managed() {
  //   let (test_dir, project_data) = base_test(Flavour::Coretto, true, None,
  // None, true, false);

  //   let runtime = tokio::runtime::Builder::new_current_thread()
  //     .enable_all()
  //     .build()
  //     .expect("Build runtime");

  //   let res = runtime.block_on(revert_jre(test_dir.path())).unwrap();

  //   assert!(res);
  //   assert!(!test_dir
  //     .path()
  //     .join(consts::JRE_PATH)
  //     .with_file_name(format!("jre_{}", Flavour::Coretto))
  //     .exists());
  //   assert!(project_data
  //     .path()
  //     .join(format!("jre_{}", Flavour::Coretto))
  //     .exists());
  //   assert!(test_dir.path().join(consts::JRE_PATH).exists());
  // }

  #[test]
  fn revert_when_original_backup_present_but_actual_missing() {
    let test_dir = TempDir::new().expect("Create tempdir");
    let target_path = test_dir.path().join(consts::JRE_PATH);

    #[cfg(target_os = "macos")]
    std::fs::create_dir_all(target_path.parent().expect("Get path parent"))
      .expect("Create parent dir");

    std::fs::create_dir_all(target_path.with_file_name(ORIGINAL_JRE_BACKUP))
      .expect("Create mock original backup");

    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .expect("Build runtime");

    let res = runtime
      .block_on(revert_jre(test_dir.path().to_owned()))
      .unwrap();

    assert!(res);
    assert!(test_dir.path().join(consts::JRE_PATH).exists());
  }

  // #[test]
  // fn use_cached_when_managed() {
  //   let flavour = Flavour::Coretto;

  //   let project_test_dir = TempDir::new().expect("Create project test dir");

  //   std::fs::create_dir_all(project_test_dir.path().join(format!("jre_{}",
  // flavour)))     .expect("Created mock cached JRE");
  //   std::fs::create_dir_all(project_test_dir.path().join(format!("jre_{}/bin",
  // flavour)))     .expect("Created mock cached JRE");
  //   std::fs::OpenOptions::new()
  //     .create_new(true)
  //     .write(true)
  //     .open(project_test_dir.path().join(format!(
  //       "jre_{}/bin/{}",
  //       flavour,
  //       if cfg!(target_os = "windows") {
  //         "java.exe"
  //       } else {
  //         "java"
  //       }
  //     )))
  //     .expect("Created mock cached JRE");

  //   base_test(flavour, true, None, Some(project_test_dir), true, false);
  // }

  // #[test]
  // fn downloads_when_managed_if_no_cache() {
  //   let flavour = Flavour::Coretto;

  //   let project_test_dir = TempDir::new().expect("Create project test dir");

  //   let (test_dir, _project_data) =
  //     base_test(flavour, true, None, Some(project_test_dir), true, false);

  //   let jre_path = test_dir.path().join(consts::JRE_PATH);
  //   assert!(jre_path.is_symlink());
  //   assert!(jre_path.join(".moss").exists());

  //   let dot_moss_string =
  //     std::fs::read_to_string(jre_path.join(".moss")).expect("Read moss
  // dotfile");   let installed_flavour =
  //     serde_json::from_str::<Flavour>(&dot_moss_string).expect("Deserialise
  // installed flavour");   assert!(installed_flavour == flavour)
  // }

  // #[test]
  // fn saves_to_cache_when_unmanaged() {
  //   let flavour = Flavour::Coretto;

  //   let project_test_dir = TempDir::new().expect("Create project test dir");

  //   let (_, project_test_dir) = base_test(flavour, true, None,
  // Some(project_test_dir), true, false);

  //   let (_, project_test_dir) = base_test(flavour, true, None,
  // Some(project_test_dir), true, false);

  //   assert!(project_test_dir
  //     .path()
  //     .join(format!("jre_{}", flavour))
  //     .exists())
  // }

  #[test]
  fn returns_early_if_flavour_already_installed() {
    let flavour = Flavour::Coretto;

    let project_test_dir = TempDir::new().expect("Create project test dir");

    std::fs::create_dir_all(project_test_dir.path().join(format!("jre_{}", flavour)))
      .expect("Created mock cached JRE");
    std::fs::create_dir_all(project_test_dir.path().join(format!("jre_{}/bin", flavour)))
      .expect("Created mock cached JRE");
    std::fs::OpenOptions::new()
      .create_new(true)
      .write(true)
      .open(project_test_dir.path().join(format!(
        "jre_{}/bin/{}",
        flavour,
        if cfg!(target_os = "windows") {
          "java.exe"
        } else {
          "java"
        }
      )))
      .expect("Created mock cached JRE");
    serde_json::to_writer(
      std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(
          project_test_dir
            .path()
            .join(format!("jre_{}/.moss", flavour)),
        )
        .expect("Created mock cached JRE"),
      &flavour,
    )
    .expect("Write installed flavour to dot file");

    base_test(flavour, true, None, Some(project_test_dir), false);
  }
}

use std::{
  collections::VecDeque,
  io::Cursor,
  path::{Path, PathBuf},
};

use anyhow::Context;
use compress_tools::uncompress_archive;
use druid::{ExtEventSink, Target, Selector};
use rand::random;
use tempfile::TempDir;
use tokio::runtime::Handle;
use strum_macros::Display;

use crate::app::App;

pub const SWAP_COMPLETE: Selector = Selector::new("settings.jre.swap_complete");

#[derive(Display)]
pub enum Flavour {
  Coretto,
  Hotspot,
  Wisp,
}

impl Flavour {
  pub async fn swap(&self, ext_ctx: ExtEventSink, root: PathBuf) {
    ext_ctx.submit_command(App::LOG_MESSAGE, format!("Beginning JRE upgrade - installing {}", self), Target::Auto).expect("Send message");

    let res = self.swap_jre(&root).await;

    match res {
      Ok(_) => ext_ctx.submit_command(App::LOG_MESSAGE, String::from("JRE upgrade complete!"), Target::Auto).expect("Send message"),
      Err(err) => ext_ctx.submit_command(App::LOG_MESSAGE, format!("ERROR: Failed to upgrade JRE. Your Starsector installation may be corrupted.\nError: {:?}", err), Target::Auto).expect("Send message")
    }
    let _ = ext_ctx.submit_command(SWAP_COMPLETE, (), Target::Auto);
  }

  async fn swap_jre(&self, root: &Path) -> anyhow::Result<()> {
    let tempdir = self.unpack(root).await?;

    let jre_8 = Self::find_jre(tempdir.path()).await?;

    let stock_jre = root.join(consts::JRE_PATH);

    let is_original = std::fs::read_to_string(stock_jre.join("release")).is_ok_and(|release| {
      release
        .split_ascii_whitespace()
        .next()
        .is_some_and(|version| version.eq_ignore_ascii_case(r#"JAVA_VERSION="1.7.0""#))
    });

    let mut backup = stock_jre.with_file_name(if is_original {
      "original_jre"
    } else {
      "backup_jre"
    });
    while backup.exists() {
      backup.set_extension(random::<u16>().to_string());
    }

    std::fs::rename(&stock_jre, backup)?;
    std::fs::rename(jre_8, &stock_jre)?;

    Ok(())
  }

  fn get_url(&self) -> String {
    match self {
      Flavour::Coretto => consts::CORETTO,
      Flavour::Hotspot => consts::HOTSPOT,
      Flavour::Wisp => consts::WISP,
    }
    .to_string()
  }

  async fn unpack(&self, root: &Path) -> anyhow::Result<TempDir> {
    let url = Self::get_url(self);

    let tempdir = TempDir::new_in(&root).context("Create tempdir")?;

    let mut res = reqwest::get(url).await?;

    let mut buf = Vec::new();
    while let Some(bytes) = res.chunk().await? {
      buf.append(&mut bytes.to_vec())
    }

    let path = root.join(tempdir.path());
    Handle::current()
      .spawn_blocking(move || -> anyhow::Result<()> {
        uncompress_archive(Cursor::new(buf), &path, compress_tools::Ownership::Ignore)
          .context("Failed to unpack")
      })
      .await??;

    Ok(tempdir)
  }

  async fn find_jre(root: &Path) -> anyhow::Result<PathBuf> {
    let mut visit = VecDeque::new();
    visit.push_back(root.to_path_buf());
    Handle::current()
      .spawn_blocking(move || {
        while let Some(path) = visit.pop_front() {
          if let Ok(mut iter) = path.read_dir() {
            while let Some(Ok(file)) = iter.next() {
              if let Ok(file_type) = file.file_type() {
                if file_type.is_dir() {
                  if cfg!(target_os = "windows") && file.file_name().eq_ignore_ascii_case("bin") {
                    return Some(
                      file
                        .path()
                        .parent()
                        .expect("Get parent of bin")
                        .to_path_buf(),
                    );
                  } else if cfg!(not(target_os = "windows"))
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
      .ok_or_else(|| anyhow::Error::msg("Could not find JRE in given folder"))
  }
}

pub async fn revert(ext_ctx: ExtEventSink, root: PathBuf) {
  ext_ctx.submit_command(App::LOG_MESSAGE, String::from("Attempting to revert to JRE 7"), Target::Auto).expect("Send message");

  let res = revert_jre(&root).await;

  match res {
    Ok(true) => ext_ctx.submit_command(App::LOG_MESSAGE, String::from("Succesfully reverted to JRE 7"), Target::Auto).expect("Send message"),
    Ok(false) => ext_ctx.submit_command(App::LOG_MESSAGE, String::from("ERROR: Could not revert to JRE 7 - no JRE 7 backup found"), Target::Auto).expect("Send message"),
    Err(err) => ext_ctx.submit_command(App::LOG_MESSAGE, format!("ERROR: Failed to revert JRE. Your Starsector installation may be corrupted.\nError: {:?}", err), Target::Auto).expect("Send message")
  }
  let _ = ext_ctx.submit_command(SWAP_COMPLETE, (), Target::Auto);
}

async fn revert_jre(root: &Path) -> anyhow::Result<bool> {
  let target = root.join(consts::JRE_PATH);
  let original = target.with_file_name("original_jre");

  if original.exists() {
    let mut backup = target.with_file_name("backup_jre");
    while backup.exists() {
      backup.set_extension(random::<u16>().to_string());
    };

    std::fs::rename(&target, backup)?;
    std::fs::rename(original, &target)?;

    Ok(true)
  } else {
    Ok(false)
  }
}

#[cfg(target_os = "windows")]
mod consts {
  pub const CORETTO: &'static str = "https://corretto.aws/downloads/resources/8.272.10.3/amazon-corretto-8.272.10.3-windows-x64-jre.zip";
  pub const HOTSPOT: &'static str = "https://github.com/AdoptOpenJDK/openjdk8-binaries/releases/download/jdk8u272-b10/OpenJDK8U-jre_x64_windows_hotspot_8u272b10.zip";
  pub const WISP: &'static str =
    "https://drive.google.com/uc?export=download&id=155Lk0ml9AUGp5NwtTZGpdu7e7Ehdyeth&confirm=t";

  pub const JRE_PATH: &'static str = "jre";
}
#[cfg(target_os = "macos")]
mod consts {
  pub const CORETTO: &'static str = "https://corretto.aws/downloads/resources/8.272.10.3/amazon-corretto-8.272.10.3-linux-x64.tar.gz";
  pub const HOTSPOT: &'static str = "https://github.com/AdoptOpenJDK/openjdk8-binaries/releases/download/jdk8u272-b10/OpenJDK8U-jre_x64_linux_hotspot_8u272b10.tar.gz";
  pub const WISP: &'static str =
    "https://drive.google.com/uc?export=download&id=1PW9v_CL719buKHe69GaN9fCXcPIqDOIi&confirm=t";

  pub const JRE_PATH: &'static str = "jre_linux";
}
#[cfg(target_os = "linux")]
mod consts {
  pub const CORETTO: &'static str = "https://corretto.aws/downloads/resources/8.272.10.3/amazon-corretto-8.272.10.3-macosx-x64.tar.gz";
  pub const HOTSPOT: &'static str = "https://github.com/AdoptOpenJDK/openjdk8-binaries/releases/download/jdk8u272-b10/OpenJDK8U-jre_x64_mac_hotspot_8u272b10.tar.gz";
  pub const WISP: &'static str =
    "https://drive.google.com/uc?export=download&id=1TRHjle6-MOpn1zJhtSA9yvwXIQip_F_n&confirm=t";

  pub const JRE_PATH: &'static str = "Contents/Home";
}

#[cfg(test)]
mod test {
  use tempfile::TempDir;

  use super::{consts, Flavour, revert_jre};

  fn base_test(flavour: Flavour, mock_original: bool) -> TempDir {
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .expect("Build runtime");

    runtime.block_on(async {
      let test_dir = TempDir::new().expect("Create tempdir");

      let target_path = test_dir.path().join(consts::JRE_PATH);
      std::fs::create_dir(&target_path).expect("Create mock JRE folder");

      if mock_original {
        std::fs::write(target_path.join("release"), r#"JAVA_VERSION="1.7.0""#).expect("Write test release");
      }

      let res = flavour.swap_jre(test_dir.path()).await;

      assert!(res.is_ok(), "{:?}", res);

      if mock_original {
        assert!(test_dir.path().join("original_jre").exists());
      } else {
        assert!(test_dir.path().join("backup_jre").exists());
      }

      assert!(target_path.exists());

      assert!(target_path.join("bin").exists());

      #[cfg(target_os = "windows")]
      assert!(target_path.join("bin/java.exe").exists());
      #[cfg(not(target_os = "windows"))]
      assert!(target_path.join("bin/java").exists());

      test_dir
    })
  }

  #[test]
  fn coretto() {
    base_test(Flavour::Coretto, true);
  }

  #[test]
  fn hotspot() {
    base_test(Flavour::Hotspot, true);
  }

  #[test]
  fn wisp() {
    base_test(Flavour::Wisp, true);
  }

  #[test]
  fn does_not_revert_when_no_original() {
    let test_dir = base_test(Flavour::Coretto, false);

    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .expect("Build runtime");

    let res = runtime.block_on(revert_jre(test_dir.path()));

    assert!(res.is_ok(), "{:?}", res);

    if let Ok(res) = res {
      assert!(!res);
      assert!(test_dir.path().join("backup_jre").exists());
      assert!(test_dir.path().join("jre").exists());
    }
  }

  #[test]
  fn revert_when_original_present() {
    let test_dir = base_test(Flavour::Coretto, true);

    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .expect("Build runtime");

    let res = runtime.block_on(revert_jre(test_dir.path()));

    assert!(res.is_ok(), "{:?}", res);

    if let Ok(res) = res {
      assert!(res);
      assert!(test_dir.path().join("backup_jre").exists());
      assert!(test_dir.path().join("jre").exists());
    }
  }
}
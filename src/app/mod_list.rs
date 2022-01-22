use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use druid::{Widget, widget::{Scroll, List, ListIter, Painter, Flex, Either, Label}, lens, WidgetExt, Data, Lens, LensExt, RenderContext, theme, Selector};
use if_chain::if_chain;
use serde::{Serialize, Deserialize};

use super::{mod_entry::{ModEntry, ModVersionMeta}, util::SaveError};

pub mod headings;

#[derive(Clone, Data, Lens)]
pub struct ModList {
  #[data(same_fn="PartialEq::eq")]
  mods: BTreeMap<String, Arc<ModEntry>>,
}

impl ModList {
  const SELECTOR: Selector<ModListCommands> = Selector::new("mod_list");

  pub fn new() -> Self {
    Self {
      mods: BTreeMap::new(),
    }
  }

  pub fn ui_builder() -> impl Widget<Self> {
    Flex::column()
      .with_child(headings::Headings::ui_builder().lens(lens::Unit))
      .with_flex_child(
        Either::new(
          |data: &ModList, _| data.mods.len() > 0,
          Scroll::new(
            List::new(|| {
              ModEntry::ui_builder().expand_width().lens(lens!((Arc<ModEntry>, usize), 0)).background(Painter::new(|ctx, (_, i), env| {
                let rect = ctx.size().to_rect();
                if i % 2 == 0 {
                  ctx.fill(rect, &env.get(theme::BACKGROUND_DARK))
                } else {
                  ctx.fill(rect, &env.get(theme::BACKGROUND_LIGHT))
                }
              }))
            }).lens(lens::Identity).background(theme::BACKGROUND_LIGHT)
          ).vertical(),
          Label::new("No mods").expand().background(theme::BACKGROUND_LIGHT)
        ),
        1.
      )
  }

  #[must_use]
  pub fn parse_mod_folder(&mut self, root_dir: &Option<PathBuf>) {
    self.mods.clear();

    if let Some(root_dir) = root_dir {
      let mod_dir = root_dir.join("mods");
      let enabled_mods_filename = mod_dir.join("enabled_mods.json");

      let enabled_mods = if !enabled_mods_filename.exists() {
        vec![]
      } else {
        if_chain! {
          if let Ok(enabled_mods_text) = std::fs::read_to_string(enabled_mods_filename);
          if let Ok(EnabledMods { enabled_mods }) = serde_json::from_str::<EnabledMods>(&enabled_mods_text);
          then {
            enabled_mods
          } else {
            return
          }
        }
      };

      if let Ok(dir_iter) = std::fs::read_dir(mod_dir) {
        let enabled_mods_iter = enabled_mods.iter();

        let (mods, versions): (Vec<(String, Arc<ModEntry>)>, Vec<Option<ModVersionMeta>>) = dir_iter
          .filter_map(|entry| entry.ok())
          .filter(|entry| {
            if let Ok(file_type) = entry.file_type() {
              file_type.is_dir()
            } else {
              false
            }
          })
          .filter_map(|entry| {
            if let Ok(mut mod_info) = ModEntry::from_file(&entry.path()) {
              mod_info.set_enabled(enabled_mods_iter.clone().find(|id| mod_info.id.clone().eq(*id)).is_some());
              Some((
                (
                  mod_info.id.clone(),
                  Arc::new(mod_info.clone())
                ),
                mod_info.version_checker.clone()
              ))
            } else {
              dbg!(entry.path());
              None
            }
          })
          .unzip();

        self.mods.extend(mods);

        // versions.iter()
        //   .filter_map(|v| v.as_ref())
        //   .map(|v| Command::perform(util::get_master_version(v.clone()), ModListMessage::MasterVersionReceived))
        //   .collect()
      } else {
        // debug_println!("Fatal. Could not parse mods folder. Alert developer");
        
      }
    } else {
      
    }
  }
}

enum ModListCommands {
  UpdateChildren(usize, f64),
  RestoreList
}

impl ListIter<(Arc<ModEntry>, usize)> for ModList {
  fn for_each(&self, mut cb: impl FnMut(&(Arc<ModEntry>, usize), usize)) {
    for (i, item) in self.mods.values().cloned().enumerate() {
      cb(&(item, i), i);
    }
  }
  
  fn for_each_mut(&mut self, mut cb: impl FnMut(&mut (Arc<ModEntry>, usize), usize)) {
    for (i, item) in self.mods.values_mut().enumerate() {
      cb(&mut (item.clone(), i), i);
    }
  }
  
  fn data_len(&self) -> usize {
    self.mods.len()
  }
}

#[derive(Serialize, Deserialize)]
pub struct EnabledMods {
  #[serde(rename = "enabledMods")]
  enabled_mods: Vec<String>
}

impl EnabledMods {
  pub async fn save(self, path: PathBuf) -> Result<(), SaveError> {
    use tokio::fs;
    use tokio::io::AsyncWriteExt;

    let json = serde_json::to_string_pretty(&self)
      .map_err(|_| SaveError::FormatError)?;

    let mut file = fs::File::create(path)
      .await
      .map_err(|_| SaveError::FileError)?;

    file.write_all(json.as_bytes())
      .await
      .map_err(|_| SaveError::WriteError)
  }
}

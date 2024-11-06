use std::{
  collections::VecDeque,
  iter::FusedIterator,
  path::{Path, PathBuf},
};

pub(crate) struct ModSearch {
  pub(crate) paths: VecDeque<PathBuf>,
}

impl Iterator for ModSearch {
  type Item = std::io::Result<PathBuf>;

  fn next(&mut self) -> Option<Self::Item> {
    while let Some(path) = self.paths.pop_front() {
      let folders = path.read_dir().map(|iter| {
        iter.filter_map(|entry| {
          entry
            .ok()
            .filter(|entry| {
              entry
                .file_type()
                .as_ref()
                .is_ok_and(std::fs::FileType::is_dir)
            })
            .map(|entry| entry.path())
        })
      });

      let mut res = None;

      match folders {
        Ok(folders) => self.paths.extend(folders),
        Err(err) => res = Some(Err(err)),
      }

      if path.join("mod_info.json").is_file() {
        res = Some(Ok(path));
      }

      if res.is_some() {
        return res;
      }
    }

    None
  }
}

#[allow(dead_code)]
impl ModSearch {
  pub fn new(path: impl AsRef<Path>) -> Self {
    let mut paths = VecDeque::new();
    paths.push_front(path.as_ref().to_path_buf());

    ModSearch { paths }
  }

  pub fn first(&mut self) -> std::io::Result<Option<PathBuf>> {
    self.next().transpose()
  }

  pub fn exhaustive(&mut self) -> std::io::Result<Vec<PathBuf>> {
    self.collect()
  }
}

impl FusedIterator for ModSearch {}

#[cfg(test)]
mod test {
  use std::{collections::HashSet, fs, ops::Deref, path::Path};

  use tempfile::{tempdir, TempDir};

  use super::ModSearch;

  fn fill_folder_with_n_mods<const N: usize>(path: impl Deref<Target = Path>) {
    for i in 0..N {
      fs::create_dir(path.join(format!("{i}"))).expect("Create fake mod dir");
      fs::File::create(path.join(format!("{i}")).join("mod_info.json"))
        .expect("Create fake mod_info.json");
    }
  }

  fn create_folder_with_n_mods<const N: usize>() -> TempDir {
    let temp_dir = tempdir().expect("Create temp dir");

    fill_folder_with_n_mods::<N>(temp_dir.path());

    temp_dir
  }

  #[test]
  fn find_first_valid_mod() {
    let mods_dir = create_folder_with_n_mods::<1>();

    let mut iter = ModSearch::new(mods_dir.path());

    assert_eq!(
      iter
        .next()
        .transpose()
        .ok()
        .flatten()
        .expect("Find first mod"),
      mods_dir.path().join("0")
    );

    assert!(iter.next().is_none());
  }

  #[test]
  fn find_all_mods() {
    let mods_dir = create_folder_with_n_mods::<5>();

    let mut iter = ModSearch::new(mods_dir.path());

    let mut path_set = HashSet::new();

    for i in 0..5 {
      let mod_path = iter
        .next()
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_else(|| panic!("Failed to find mod {}", i));

      assert!(mod_path.starts_with(mods_dir.path()));

      path_set.insert(mod_path);
    }

    assert!(iter.next().is_none());
    assert_eq!(path_set.len(), 5);
  }

  #[test]
  fn find_all_nested_mods() {
    let mods_dir = TempDir::new().unwrap();

    let nested = mods_dir.path().join(format!("nested_{}", 0));
    fs::create_dir(&nested).unwrap();
    fill_folder_with_n_mods::<2>(nested);
    let nested = mods_dir.path().join(format!("nested_{}", 1));
    fs::create_dir(&nested).unwrap();
    fill_folder_with_n_mods::<2>(nested);

    let mut iter = ModSearch::new(mods_dir.path());

    let mut path_set = HashSet::new();

    for i in 0..4 {
      let mod_path = iter
        .next()
        .transpose()
        .ok()
        .flatten()
        .unwrap_or_else(|| panic!("Failed to find mod {}", i));

      assert!(mod_path.starts_with(mods_dir.path()));

      path_set.insert(mod_path);
    }

    assert!(iter.next().is_none());
    assert_eq!(path_set.len(), 4);
  }
}

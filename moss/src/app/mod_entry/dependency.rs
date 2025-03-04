use std::fmt::Display;

  use druid::Data;
  use fake::Dummy;
  use serde::Deserialize;

  use crate::app::mod_entry::Version;

  #[derive(Debug, Clone, PartialEq, Data, Deserialize, Dummy)]
  pub struct Dependency {
pub id: String,
pub name: Option<String>,
pub version: Option<Version>,
  }

  impl Display for Dependency {
fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
  let Dependency { id, name, version } = self;
  write!(f, "{}", name.as_ref().unwrap_or(id))?;
  if let Some(version) = version {
    write!(f, "@{version}")?;
  }

  Ok(())
}
  }

  #[cfg(test)]
  mod test {
use crate::app::{
  mod_entry::{dependency::Dependency, Version, VersionComplex, ViewModEntry},
  mod_list::ModMap,
};

#[test]
fn enable_dependencies_minimal() {
  // Setup
  let dep = Dependency {
    id: "Dep".to_owned(),
    name: None,
    version: Some(Version::Complex(VersionComplex {
      major: 1,
      minor: 0,
      patch: 0.to_string(),
    })),
  };
  let dep_entry = ViewModEntry {
    mod_id: "Dep".to_owned(),
    version: Version::Complex(VersionComplex {
      major: 1,
      minor: 0,
      patch: 0.to_string(),
    }),
    ..Default::default()
  };
  let entry = ViewModEntry {
    mod_id: "Entry".to_owned(),
    dependencies: vec![dep].into(),
    ..Default::default()
  };

  let mut mods = ModMap::new();
  mods.extend([
    ("Dep".to_owned(), dep_entry.into()),
    ("Entry".to_owned(), entry.into()),
  ]);

  // Assert
  assert!(!mods["Entry"].enabled);
  assert!(!mods["Dep"].enabled);

  assert!(ViewModEntry::enable_all_dependencies("Entry", &mut mods));

  assert!(mods["Entry"].enabled);
  assert!(mods["Dep"].enabled);
}

#[test]
fn enable_dependencies() {
  // Setup
  let sub_dep_entry = ViewModEntry {
    mod_id: "subdep".to_owned(),
    version: Version::Complex(VersionComplex {
      major: 0,
      minor: 2,
      patch: "some-RC99".to_owned(),
    }),
    ..Default::default()
  };
  let dep_entry = ViewModEntry {
    mod_id: "Dep".to_owned(),
    version: Version::Complex(VersionComplex {
      major: 1,
      minor: 0,
      patch: 0.to_string(),
    }),
    dependencies: vec![Dependency {
      id: "subdep".to_owned(),
      name: None,
      version: Some(Version::Complex(VersionComplex {
        major: 0,
        minor: 5,
        patch: "foo".to_owned(),
      })),
    }]
    .into(),
    ..Default::default()
  };
  let entry = ViewModEntry {
    mod_id: "Entry".to_owned(),
    dependencies: vec![Dependency {
      id: "Dep".to_owned(),
      name: None,
      version: Some(Version::Complex(VersionComplex {
        major: 1,
        minor: 0,
        patch: 0.to_string(),
      })),
    }]
    .into(),
    ..Default::default()
  };

  let unused_entry_a = ViewModEntry {
    mod_id: "Unused A".to_owned(),
    ..Default::default()
  };
  let unused_entry_b = ViewModEntry {
    mod_id: "Unused B".to_owned(),
    ..Default::default()
  };

  let mut mods = ModMap::new();
  mods.extend([
    ("Dep".to_owned(), dep_entry.into()),
    ("Entry".to_owned(), entry.into()),
    ("subdep".to_owned(), sub_dep_entry.into()),
    ("Unused A".to_owned(), unused_entry_a.into()),
    ("Unused B".to_owned(), unused_entry_b.into()),
  ]);

  // Assert
  assert!(mods.values().all(|entry| !entry.enabled));

  assert!(ViewModEntry::enable_all_dependencies("Entry", &mut mods));

  assert!(mods["Entry"].enabled);
  assert!(mods["Dep"].enabled);
  assert!(mods["subdep"].enabled);
  assert!(!mods["Unused A"].enabled);
  assert!(!mods["Unused B"].enabled);
}

#[test]
fn missing_dependency() {
  let entry = ViewModEntry {
    mod_id: "entry".to_owned(),
    dependencies: vec![Dependency {
      id: "doesn't exist".to_owned(),
      name: None,
      version: None,
    }]
    .into(),
    ..Default::default()
  };

  let mut mods = ModMap::new();
  mods.extend([("entry".to_owned(), entry.into())]);

  assert!(!ViewModEntry::enable_all_dependencies("entry", &mut mods));
  assert!(!mods["entry"].enabled);
}
  }

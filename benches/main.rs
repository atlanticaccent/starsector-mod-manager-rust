#![feature(test)]

#[cfg(test)]
mod test {
  extern crate test;
  use fake::{Fake, Faker};
  use test::Bencher;

  use starsector_mod_manager::app::{
    mod_entry::{ModEntry, ViewModEntry},
    mod_list::{headings::{Header, Heading}, ModList},
    util::xxHashMap,
  };

  #[bench]
  fn benchmark_sort(b: &mut Bencher) {
    let mods: Vec<_> = (0..100)
      .map(|_| {
        let entry = Faker.fake::<ModEntry>();
        (entry.id.clone(), entry.into())
      })
      .collect();
    let mods: xxHashMap<String, ViewModEntry> = xxHashMap::from(mods);

    b.iter(|| {
      let mods = test::black_box(mods.clone());

      let mut header: Header = Default::default();
      header.sort_by.0 = Heading::Score;

      ModList::sorted_vals_inner(
        mods.clone(),
        header,
        5.fake(),
        Default::default(),
      )
    })
  }
}

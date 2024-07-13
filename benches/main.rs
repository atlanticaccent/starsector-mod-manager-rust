#![feature(test)]

mod env;

#[cfg(test)]
mod test {
  extern crate test;
  use std::{hint::black_box, sync::Arc};

  use druid::{Color, Data, Env, Key};
  use fake::{Dummy, Fake, Faker};
  use moss::app::{
    mod_entry::{ModEntry, ViewModEntry},
    mod_list::{
      headings::{Header, Heading},
      ModList,
    },
    util::FastImMap,
  };
  use test::Bencher;

  use crate::set_key;

  #[bench]
  fn benchmark_sort(b: &mut Bencher) {
    let mods: Vec<_> = (0..100)
      .map(|_| {
        let entry = Faker.fake::<ModEntry>();
        (entry.id.clone(), Arc::new(entry.into()))
      })
      .collect();
    let mods: FastImMap<String, Arc<ViewModEntry>> = FastImMap::from(mods);

    b.iter(|| {
      let mods = test::black_box(mods.clone());

      let mut header: Header = Default::default();
      header.sort_by.0 = Heading::Score;

      ModList::sorted_vals_inner(mods.clone(), header, 5.fake(), Default::default())
    })
  }

  #[bench]
  fn benchmark_env(b: &mut Bencher) {
    fn f<T: Dummy<Faker>>() -> T {
      Faker.fake()
    }

    let mut env = Env::empty();
    (0..1000).for_each(|_| {
      let key: String = (50..100).fake();
      let key = Box::leak(key.into_boxed_str());
      set_key!(env, key, Color, Color::from_rgba32_u32(f()));

      // match (0..11).fake::<usize>() {
      //   0 => set_key!(env, key, bool),
      //   1 => ,
      //   2 => set_key!(env, key, f64),
      //   3 => set_key!(env, key, Point, Point::new(f(), f())),
      //   4 => set_key!(env, key, Size, Size::new(f(), f())),
      //   5 => set_key!(env, key, Rect, Rect::new(f(), f(), f(), f())),
      //   6 => set_key!(env, key, Insets, Insets::new(f(), f(), f(), f())),
      //   7 => set_key!(env, key, u64),
      //   8 => {
      //     let arc_str: Arc<str> = (50..100).fake::<String>().into();
      //     set_key!(env, key, ArcStr, arc_str)
      //   }
      //   9 => set_key!(env, key, FontDescriptor, FontDescriptor::default()),
      //   10 => set_key!(
      //     env,
      //     key,
      //     RoundedRectRadii,
      //     RoundedRectRadii::from_single_radius(f())
      //   ),
      //   _ => return,
      // }
    });

    let key: Key<Color> = Key::new(Box::leak(100.fake::<String>().into_boxed_str()));
    env.set(key.clone(), Color::from_rgba32_u32(f()));

    b.iter(|| {
      let mut new_env = black_box(env.clone());
      new_env.set(key.clone(), Color::from_rgba32_u32(f()));

      black_box(env.same(&new_env))
    })
  }
}

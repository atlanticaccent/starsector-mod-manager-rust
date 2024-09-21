#[macro_export]
macro_rules! set_key {
  ($env:ident, $key:ident, $value:ty) => {
    $env.set(
      druid::Key::<$value>::new($key),
      fake::Faker.fake::<$value>(),
    )
  };
  ($env:ident, $key:ident, $type:ty, $value:expr) => {
    $env.set(druid::Key::<$type>::new($key), $value)
  };
}

use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub(crate) enum Json {
  Null,
  Boolean(bool),
  Str(String),
  Num(Number),
  Array(Vec<Json>),
  Map(HashMap<String, Json>),
}

#[derive(Debug, PartialEq)]
pub(crate) enum Float {
  F32(f32),
  F64(f64),
}

#[derive(Debug, PartialEq)]
pub(crate) enum UnsignNumber {
  U8(u8),
  U16(u16),
  U32(u32),
  U64(u64),
  U128(u128),
}

#[derive(Debug, PartialEq)]
pub(crate) enum SignedNumber {
  I8(i8),
  I16(i16),
  I32(i32),
  I64(i64),
  I128(i128),
}

#[derive(Debug, PartialEq)]
pub(crate) enum Number {
  Unsign(UnsignNumber),
  Signed(SignedNumber),
  Float(Float),
}

#[derive(Debug, PartialEq)]
pub(crate) struct _PrivateMarker(bool);

impl _PrivateMarker {
  pub(crate) const TRUE: Self = _PrivateMarker(true);
  pub(crate) const FALSE: Self = _PrivateMarker(false);
}

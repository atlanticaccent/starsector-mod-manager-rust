use serde::{
  Deserialize,
  de::{
    IntoDeserializer,
    value::{MapDeserializer, SeqDeserializer},
  },
  forward_to_deserialize_any,
};
use winnow::{ModalResult, Parser};

use crate::json::{
  error::Error,
  parser::LenientJsonParser,
  types::{Float, Json, Number, SignedNumber, UnsignNumber},
};

pub fn from_str<'a, T>(s: &'a str) -> Result<T, Error>
where
  T: Deserialize<'a>,
{
  let mut input = s;
  let deserializer: ModalResult<Json> =
    LenientJsonParser::stripped_json_value.parse_next(&mut input);
  T::deserialize(deserializer?)
}

impl<'de, 'a> serde::Deserializer<'de> for Json {
  type Error = Error;

  fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
  where
    V: serde::de::Visitor<'de>,
  {
    match self {
      Json::Null => visitor.visit_unit(),
      Json::Boolean(bool) => visitor.visit_bool(bool),
      Json::Str(string) => visitor.visit_string(string),
      Json::Num(num) => visit_number(visitor, num),
      Json::Array(array) => visitor.visit_seq(SeqDeserializer::new(array.into_iter())),
      Json::Map(map) => visitor.visit_map(MapDeserializer::new(map.into_iter())),
    }
  }

  forward_to_deserialize_any! {
    bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes
    byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct map
    struct enum identifier ignored_any
  }
}

pub(crate) fn visit_number<'de, V: serde::de::Visitor<'de>>(
  visitor: V,
  num: Number,
) -> Result<V::Value, Error> {
  match num {
    Number::Unsign(unsign_number) => visit_unsign(visitor, unsign_number),
    Number::Signed(signed_number) => visit_signed(visitor, signed_number),
    Number::Float(float) => visit_float(visitor, float),
  }
}

fn visit_float<'de, V: serde::de::Visitor<'de>>(
  visitor: V,
  float: Float,
) -> Result<V::Value, Error> {
  match float {
    Float::F32(v) => visitor.visit_f32(v),
    Float::F64(v) => visitor.visit_f64(v),
  }
}

fn visit_signed<'de, V: serde::de::Visitor<'de>>(
  visitor: V,
  signed_number: SignedNumber,
) -> Result<V::Value, Error> {
  match signed_number {
    SignedNumber::I8(v) => visitor.visit_i8(v),
    SignedNumber::I16(v) => visitor.visit_i16(v),
    SignedNumber::I32(v) => visitor.visit_i32(v),
    SignedNumber::I64(v) => visitor.visit_i64(v),
    SignedNumber::I128(v) => visitor.visit_i128(v),
  }
}

fn visit_unsign<'de, V: serde::de::Visitor<'de>>(
  visitor: V,
  unsign_number: UnsignNumber,
) -> std::result::Result<V::Value, Error> {
  match unsign_number {
    UnsignNumber::U8(v) => visitor.visit_u8(v),
    UnsignNumber::U16(v) => visitor.visit_u16(v),
    UnsignNumber::U32(v) => visitor.visit_u32(v),
    UnsignNumber::U64(v) => visitor.visit_u64(v),
    UnsignNumber::U128(v) => visitor.visit_u128(v),
  }
}

impl<'de> IntoDeserializer<'de, Error> for Json {
  type Deserializer = Self;

  fn into_deserializer(self) -> Self::Deserializer {
    self
  }
}

#[cfg(test)]
mod test {
  use std::collections::HashMap;

  use serde::Deserialize;

  use crate::json::deser::from_str;

  #[test]
  fn test_complex_deser() {
    let input = r#"
        #
#
    {
      "null" : null, # asdasd asdasd
      "true"  :true ,
      #asdasdasd
      "false":  false  ,
      "float" : 123e4 ,
      #asdasdasdasdasd
      "uint"  : 123,
      "int"  :-2147483647,
      "string" : " abc 123 " ,
      "array" : [ "false" , "1" , "two" ] ,
      "object" : { "a" : 1.0 , "b" : 2.0 } ,
      "empty_array" : [  ] , ####
      ## ## ## asdasdasd #@ /asf\asd
      "empty_object" : {   },
      "variant1" : 0,
      "variant2" : "Foo",
      "variant3" : {
        "a" : "Bar",
        "b" : 1
      }
    }
    #####
    "#;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(untagged)]
    enum UntaggedEnum {
      Usize(usize),
      String(String),
      Struct { a: String, b: usize },
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestStruct {
      null: Option<()>,
      r#true: bool,
      r#false: bool,
      float: f32,
      uint: usize,
      int: i32,
      string: String,
      array: Vec<String>,
      object: HashMap<String, f32>,
      empty_array: Vec<()>,
      empty_object: HashMap<String, ()>,
      variant1: UntaggedEnum,
      variant2: UntaggedEnum,
      variant3: UntaggedEnum,
    }

    let parsed_struct = from_str::<TestStruct>(input).unwrap();

    assert_eq!(
      TestStruct {
        null: None,
        r#true: true,
        r#false: false,
        float: 123_0000.0,
        uint: 123,
        int: -2147483647,
        string: String::from(" abc 123 "),
        array: vec!["false".into(), "1".into(), "two".into()],
        object: vec![("a".to_owned(), 1.0), ("b".to_owned(), 2.0)]
          .into_iter()
          .collect(),
        empty_array: Vec::new(),
        empty_object: HashMap::new(),
        variant1: UntaggedEnum::Usize(0),
        variant2: UntaggedEnum::String("Foo".to_owned()),
        variant3: UntaggedEnum::Struct {
          a: "Bar".to_owned(),
          b: 1
        }
      },
      parsed_struct,
    )
  }
}

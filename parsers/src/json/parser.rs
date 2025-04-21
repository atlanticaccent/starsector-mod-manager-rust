use std::{collections::HashMap, marker::PhantomData, str};

use winnow::{
  Result,
  ascii::{alphanumeric1, dec_int, dec_uint, float as wfloat, multispace0, till_line_ending},
  combinator::{
    Alt, alt, delimited, dispatch, empty, fail, opt, peek, repeat, separated, separated_pair, trace,
  },
  error::{AddContext, ContextError, ParserError, StrContext, StrContextValue},
  prelude::*,
  token::{any, none_of, take},
};

use crate::json::{
  CommentDefinition,
  traits::InputStream,
  types::{_PrivateMarker, Float, Json, Number, SignedNumber, UnsignNumber},
};

#[derive(Default)]
pub struct JsonParser<const S: usize, const L: usize, T = NoComment>(PhantomData<T>);

#[allow(dead_code)]
pub type StrictJsonParser = JsonParser<0, 0, NoComment>;
pub type LenientJsonParser = JsonParser<0, 0, LenientComment>;

pub struct LenientComment;

impl CommentDefinition<0, 0> for LenientComment {
  const SHORT_PREFIX: [char; 0] = [];
  const LONG_PREFIX: [&'static str; 0] = [];

  type Parser = JsonParser<1, 1, Self>;

  fn match_comment_prefix<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> std::result::Result<&'i str, E> {
    (
      dispatch!(any;
        '#' => empty,
        '/' => '/'.void(),
        _ => fail
      ),
      till_line_ending,
    )
      .take()
      .context(StrContext::Label("comment"))
      .context(StrContext::Expected(StrContextValue::CharLiteral('#')))
      .context(StrContext::Expected(StrContextValue::StringLiteral("//")))
      .parse_next(input)
  }
}

pub struct NoComment;

impl CommentDefinition<0, 0> for NoComment {
  const SHORT_PREFIX: [char; 0] = [];
  const LONG_PREFIX: [&'static str; 0] = [];
  #[allow(private_interfaces)]
  const _COMMENTS_ENABLED: _PrivateMarker = _PrivateMarker::TRUE;

  type Parser = JsonParser<0, 0, Self>;
}

impl<const S: usize, const L: usize, T: CommentDefinition<S, L>> JsonParser<S, L, T> {
  pub(crate) fn trailing_whitespace<
    'i,
    I: InputStream<'i>,
    E: ParserError<I> + AddContext<I, StrContext>,
  >(
    input: &mut I,
  ) -> Result<(&'i str, Option<char>, &'i str), E> {
    (Self::ws, opt(','), Self::ws).parse_next(input)
  }

  pub(crate) fn stripped_value<
    'i,
    I: InputStream<'i>,
    O,
    E: ParserError<I> + AddContext<I, StrContext>,
  >(
    parser: impl Parser<I, O, E>,
  ) -> impl Parser<I, O, E> {
    delimited(Self::ws, parser, Self::trailing_whitespace)
      .context(StrContext::Label("strip leading and trailing text"))
  }

  pub(crate) fn stripped_json_value<
    'i,
    I: InputStream<'i>,
    E: ParserError<I> + AddContext<I, StrContext>,
  >(
    input: &mut I,
  ) -> Result<Json, E> {
    Self::stripped_value(Self::json_value)
      .context(StrContext::Label("strip outer json"))
      .parse_next(input)
  }

  pub(crate) fn json_value<
    'i,
    I: InputStream<'i>,
    E: ParserError<I> + AddContext<I, StrContext>,
  >(
    input: &mut I,
  ) -> Result<Json, E> {
    dispatch!(peek(any);
      'n' => Self::null.map(|_| Json::Null),
      't' => Self::true_.map(Json::Boolean),
      'f' => Self::false_.map(Json::Boolean),
      '"' => Self::string.map(Json::Str),
      '+' => Self::num.map(Json::Num),
      '-' => Self::neg_num.map(Json::Num),
      '0'..='9' => Self::num.map(Json::Num),
      '[' => Self::array.map(Json::Array),
      '{' => Self::object.map(Json::Map),
      _ => fail,
    )
    .context(StrContext::Label("json value"))
    .parse_next(input)
  }

  fn float<
    'i,
    I: InputStream<'i>,
    E: ParserError<I> + AddContext<I, StrContext>,
    E2: ParserError<&'i str> + AddContext<I, StrContext>,
  >(
    alternatives: impl Alt<&'i str, Number, E2>,
  ) -> impl Parser<I, Number, E> {
    let mut alternatives = peek(alt(alternatives).with_taken());
    move |input: &mut I| {
      let float_res = peek(
        alt((
          wfloat::<I, f32, E>.map(Float::F32),
          wfloat::<I, f64, E>.map(Float::F64),
        ))
        .with_taken(),
      )
      .parse_next(input);

      match float_res {
        Ok((float, mut taken)) => {
          let alt_parse = alternatives.parse_next(&mut taken);

          if let Ok((alt_num, alt_taken)) = alt_parse {
            if taken == alt_taken {
              take(taken.len()).parse_next(input)?;
              return Ok(alt_num);
            }
          }

          take(taken.len()).parse_next(input)?;
          Ok(Number::Float(float))
        }
        Err(_) => alternatives
          .parse_next(&mut input.peek_finish())
          .map(|(num, _)| num)
          .map_err(|_| E::from_input(input)),
      }
    }
  }

  fn unsigned<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<UnsignNumber, E> {
    fn dec<
      'i,
      In: InputStream<'i>,
      I: winnow::ascii::Uint,
      E: ParserError<In> + AddContext<In, StrContext>,
    >(
      input: &mut In,
    ) -> Result<I, E> {
      dec_uint.parse_next(input)
    }

    alt((
      dec::<I, u8, E>.map(UnsignNumber::U8),
      dec::<I, u16, E>.map(UnsignNumber::U16),
      dec::<I, u32, E>.map(UnsignNumber::U32),
      dec::<I, u64, E>.map(UnsignNumber::U64),
      dec::<I, u128, E>.map(UnsignNumber::U128),
    ))
    .parse_next(input)
  }

  fn signed<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<SignedNumber, E> {
    fn dec<
      'i,
      In: InputStream<'i>,
      O: winnow::ascii::Int,
      E: ParserError<In> + AddContext<In, StrContext>,
    >(
      input: &mut In,
    ) -> Result<O, E> {
      dec_int.parse_next(input)
    }

    alt((
      dec::<I, i8, E>.map(SignedNumber::I8),
      dec::<I, i16, E>.map(SignedNumber::I16),
      dec::<I, i32, E>.map(SignedNumber::I32),
      dec::<I, i64, E>.map(SignedNumber::I64),
      dec::<I, i128, E>.map(SignedNumber::I128),
    ))
    .parse_next(input)
  }

  fn neg_num<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<Number, E> {
    Self::float((Self::signed::<_, ContextError<_>>.map(Number::Signed),)).parse_next(input)
  }

  fn num<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<Number, E> {
    Self::float((
      Self::unsigned::<_, ContextError<_>>.map(Number::Unsign),
      Self::signed.map(Number::Signed),
    ))
    .parse_next(input)
  }

  /// `literal(string)` generates a parser that takes the argument string.
  ///
  /// This also shows returning a sub-slice of the original input
  fn null<'i, I: InputStream<'i>, E: ParserError<I>>(input: &mut I) -> Result<(), E> {
    // This is a parser that returns `"null"` if it sees the string "null", and
    // an error otherwise
    "null".void().parse_next(input)
  }

  /// We can combine `tag` with other functions, like `value` which returns a
  /// given constant value on success.
  fn true_<'i, I: InputStream<'i>, E: ParserError<I>>(input: &mut I) -> Result<bool, E> {
    // This is a parser that returns `true` if it sees the string "true", and
    // an error otherwise
    "true".value(true).parse_next(input)
  }

  /// We can combine `tag` with other functions, like `value` which returns a
  /// given constant value on success.
  fn false_<'i, I: InputStream<'i>, E: ParserError<I>>(input: &mut I) -> Result<bool, E> {
    // This is a parser that returns `false` if it sees the string "false", and
    // an error otherwise
    "false".value(false).parse_next(input)
  }

  /// This parser gathers all `char`s up into a `String`with a parse to take the
  /// double quote character, before the string (using `preceded`) and after the
  /// string (using `terminated`).
  fn string<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<String, E> {
    delimited(
      '\"',
      repeat(0.., Self::character).fold(String::new, |mut string, c| {
        string.push(c);
        string
      }),
      '\"',
    )
    // `context` lets you add a static string to errors to provide more information in the
    // error chain (to indicate which parser had an error)
    .context(StrContext::Expected(StrContextValue::Description("string")))
    .parse_next(input)
  }

  /// You can mix the above declarative parsing with an imperative style to
  /// handle more unique cases, like escaping
  fn character<'i, I: InputStream<'i>, E: ParserError<I>>(input: &mut I) -> Result<char, E> {
    let c = none_of('\"').parse_next(input)?;
    if c == '\\' {
      dispatch!(any;
        '"' => empty.value('"'),
        '\\' => empty.value('\\'),
        '/'  => empty.value('/'),
        'b' => empty.value('\x08'),
        'f' => empty.value('\x0C'),
        'n' => empty.value('\n'),
        'r' => empty.value('\r'),
        't' => empty.value('\t'),
        'u' => Self::unicode_escape,
        _ => fail,
      )
      .parse_next(input)
    } else {
      Ok(c)
    }
  }

  fn unicode_escape<'i, I: InputStream<'i>, E: ParserError<I>>(input: &mut I) -> Result<char, E> {
    alt((
      // Not a surrogate
      Self::u16_hex
        .verify(|cp| !(0xD800..0xE000).contains(cp))
        .map(|cp| cp as u32),
      // See https://en.wikipedia.org/wiki/UTF-16#Code_points_from_U+010000_to_U+10FFFF for details
      separated_pair(Self::u16_hex, "\\u", Self::u16_hex)
        .verify(|(high, low)| (0xD800..0xDC00).contains(high) && (0xDC00..0xE000).contains(low))
        .map(|(high, low)| {
          let high_ten = (high as u32) - 0xD800;
          let low_ten = (low as u32) - 0xDC00;
          (high_ten << 10) + low_ten + 0x10000
        }),
    ))
    .verify_map(
      // Could be probably replaced with .unwrap() or _unchecked due to the verify checks
      std::char::from_u32,
    )
    .parse_next(input)
  }

  fn u16_hex<'i, I: InputStream<'i>, E: ParserError<I>>(input: &mut I) -> Result<u16, E> {
    take(4usize)
      .verify_map(|s| u16::from_str_radix(s, 16).ok())
      .parse_next(input)
  }

  /// Some combinators, like `separated` or `repeat`, will call a parser
  /// repeatedly, accumulating results in a `Vec`, until it encounters an error.
  /// If you want more control on the parser application, check out the
  /// `iterator` combinator (cf `examples/iterator.rs`)
  fn array<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<Vec<Json>, E> {
    delimited(
      ('[', Self::ws),
      separated(0.., Self::json_value, (Self::ws, ',', Self::ws)),
      (Self::ws, opt(','), Self::ws, ']'),
    )
    .context(StrContext::Expected("array".into()))
    .parse_next(input)
  }

  fn object<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<HashMap<String, Json>, E> {
    trace(
      "object",
      delimited(
        ('{', Self::ws),
        separated(0.., Self::key_value, (Self::ws, ',', Self::ws)),
        (Self::ws, opt(','), Self::ws, '}'),
      )
      .context(StrContext::Label("object"))
      .context(StrContext::Expected(StrContextValue::Description(
        "curled braces delimiting a sequence of ws, (un)quoted string, ws, colon, ws, json, ws, \
         comma, ws",
      ))),
    )
    .parse_next(input)
  }

  fn key_value<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<(String, Json), E> {
    separated_pair(
      alt((Self::string, Self::key)),
      (Self::ws, ':', Self::ws),
      Self::json_value,
    )
    .context(StrContext::Label("key_value"))
    .parse_next(input)
  }

  fn key<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<String, E> {
    repeat(0.., alphanumeric1)
      .map(|()| ())
      .take()
      .parse_to()
      .context(StrContext::Label("key"))
      .context(StrContext::Expected(StrContextValue::Description(
        "unquoted string",
      )))
      .parse_next(input)
  }

  fn ws<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<&'i str, E> {
    (
      multispace0,
      repeat(0.., (Self::comment, multispace0)).map(|()| ()),
    )
      .take()
      .context(StrContext::Label("whitespace"))
      .context(StrContext::Expected(StrContextValue::Description(
        "0+ whitespace characters, followed by 0+ comments, followed by 0+ whitespace characters",
      )))
      .parse_next(input)
  }

  fn comment<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<&'i str, E> {
    T::match_comment_prefix.parse_next(input)
  }
}

#[cfg(test)]
mod test {
  use std::collections::HashMap;

  use winnow::Parser;

  #[allow(clippy::useless_attribute)]
  #[allow(unused_imports)] // its dead for benches
  use crate::json::{
    parser::LenientJsonParser,
    types::{Float, Json, Number, SignedNumber, UnsignNumber},
  };

  #[allow(clippy::useless_attribute)]
  #[allow(dead_code)] // its dead for benches
  type Error = winnow::error::TreeError<&'static str>;

  type JsonParser = LenientJsonParser;

  fn string() -> impl Parser<&'static str, String, Error> {
    JsonParser::string
  }

  #[test]
  fn json_string() {
    assert_eq!(string().parse_peek("\"\"").unwrap(), ("", "".to_owned()));
    assert_eq!(
      string().parse_peek("\"abc\"").unwrap(),
      ("", "abc".to_owned())
    );
    assert_eq!(
      string()
        .parse_peek("\"abc\\\"\\\\\\/\\b\\f\\n\\r\\t\\u0001\\u2014\u{2014}def\"")
        .unwrap(),
      ("", "abc\"\\/\x08\x0C\n\r\t\x01——def".to_owned())
    );
    assert_eq!(
      string().parse_peek("\"\\uD83D\\uDE10\"").unwrap(),
      ("", "😐".to_owned())
    );

    assert!(string().parse_peek("\"").is_err());
    assert!(string().parse_peek("\"abc").is_err());
    assert!(string().parse_peek("\"\\\"").is_err());
    assert!(string().parse_peek("\"\\u123\"").is_err());
    assert!(string().parse_peek("\"\\uD800\"").is_err());
    assert!(string().parse_peek("\"\\uD800\\uD800\"").is_err());
    assert!(string().parse_peek("\"\\uDC00\"").is_err());
  }

  fn stripped_json_value() -> impl Parser<&'static str, Json, Error> {
    JsonParser::stripped_json_value
  }

  #[test]
  fn json_object() {
    let input = r#"{"a":42,"b":"x"}"#;

    let expected = Json::Map(
      vec![
        (
          "a".to_owned(),
          Json::Num(Number::Unsign(UnsignNumber::U8(42))),
        ),
        ("b".to_owned(), Json::Str("x".to_owned())),
      ]
      .into_iter()
      .collect(),
    );

    assert_eq!(
      stripped_json_value().parse_peek(input).unwrap(),
      ("", expected)
    );
  }

  #[test]
  fn json_object_trailing_comma() {
    use Json::{Map, Num, Str};

    let input = r#"{"a":42,"b":"x",},"#;

    let expected = Map(
      vec![
        ("a".to_owned(), Num(Number::Unsign(UnsignNumber::U8(42)))),
        ("b".to_owned(), Str("x".to_owned())),
      ]
      .into_iter()
      .collect(),
    );

    assert_eq!(
      stripped_json_value().parse_peek(input).unwrap(),
      ("", expected)
    );
  }

  #[test]
  fn json_array() {
    use Json::{Array, Num, Str};

    let input = r#"[42,"x"]"#;

    let expected = Array(vec![
      Num(Number::Unsign(UnsignNumber::U8(42))),
      Str("x".to_owned()),
    ]);

    assert_eq!(
      stripped_json_value().parse_peek(input).unwrap(),
      ("", expected)
    );
  }

  #[test]
  fn json_array_trailing_comma() {
    use Json::{Array, Num, Str};

    let input = r#"[42,"x",]"#;

    let expected = Array(vec![
      Num(Number::Unsign(UnsignNumber::U8(42))),
      Str("x".to_owned()),
    ]);

    assert_eq!(
      stripped_json_value().parse_peek(input).unwrap(),
      ("", expected)
    );
  }

  #[test]
  fn json_whitespace() {
    use Json::{Array, Boolean, Map, Null, Num, Str};

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
    "array" : [ false , 1 , "two" ] ,
    "object" : { "a" : 1.0 , "b" : "c" } ,
    "empty_array" : [  ] , ####
    ## ## ## asdasdasd #@ /asf\asd
    "empty_object" : {   }
  }
  #####
  "#;

    assert_eq!(
      stripped_json_value().parse_peek(input).unwrap(),
      (
        "",
        Map(
          vec![
            ("null".to_owned(), Null),
            ("true".to_owned(), Boolean(true)),
            ("false".to_owned(), Boolean(false)),
            ("float".to_owned(), Num(Number::Float(Float::F32(123e4)))),
            (
              "uint".to_owned(),
              Num(Number::Unsign(UnsignNumber::U8(123)))
            ),
            (
              "int".to_owned(),
              Num(Number::Signed(SignedNumber::I32(-2147483647)))
            ),
            ("string".to_owned(), Str(" abc 123 ".to_owned())),
            (
              "array".to_owned(),
              Array(vec![
                Boolean(false),
                Num(Number::Unsign(UnsignNumber::U8(1))),
                Str("two".to_owned())
              ])
            ),
            (
              "object".to_owned(),
              Map(
                vec![
                  ("a".to_owned(), Num(Number::Float(Float::F32(1.0)))),
                  ("b".to_owned(), Str("c".to_owned())),
                ]
                .into_iter()
                .collect()
              )
            ),
            ("empty_array".to_owned(), Array(vec![]),),
            ("empty_object".to_owned(), Map(HashMap::new()),),
          ]
          .into_iter()
          .collect()
        )
      )
    );
  }
}

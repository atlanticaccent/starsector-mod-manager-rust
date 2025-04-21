use winnow::{
  Parser as _,
  ascii::Caseless,
  combinator::{alt, empty},
  error::{AddContext, ParserError, StrContext},
  stream::{AsBStr, Compare, FindSlice, ParseSlice, Stream, StreamIsPartial},
};

use crate::json::types::_PrivateMarker;

pub trait CommentDefinition<const S: usize, const L: usize> {
  const SHORT_PREFIX: [char; S];
  const LONG_PREFIX: [&'static str; L];
  #[doc(hidden)]
  #[allow(private_interfaces)]
  const _COMMENTS_ENABLED: _PrivateMarker = _PrivateMarker::TRUE;

  #[doc(hidden)]
  #[allow(private_bounds)]
  type Parser: crate::json::sealed::Sealed;

  #[doc(hidden)]
  fn match_comment_prefix<'i, I: InputStream<'i>, E: ParserError<I> + AddContext<I, StrContext>>(
    input: &mut I,
  ) -> Result<&'i str, E> {
    if let _PrivateMarker::FALSE = Self::_COMMENTS_ENABLED {
      empty.take().parse_next(input)
    } else {
      alt((alt(Self::SHORT_PREFIX).take(), alt(Self::LONG_PREFIX))).parse_next(input)
    }
  }
}

pub trait InputStream<'i>:
  StreamIsPartial
  + Stream<Token = char, Slice = &'i str, IterOffsets: Clone>
  + Compare<&'static str>
  + Compare<char>
  + Compare<Caseless<&'static str>>
  + Compare<char>
  + AsBStr
  + PartialEq
  + ParseSlice<f32>
  + ParseSlice<f64>
  + Clone
  + FindSlice<(char, char)>
{
}

impl<
  'i,
  T: StreamIsPartial
    + Stream<Token = char, Slice = &'i str, IterOffsets: Clone>
    + Compare<&'static str>
    + Compare<char>
    + Compare<Caseless<&'static str>>
    + Compare<char>
    + AsBStr
    + PartialEq
    + ParseSlice<f32>
    + ParseSlice<f64>
    + Clone
    + FindSlice<(char, char)>,
> InputStream<'i> for T
{
}

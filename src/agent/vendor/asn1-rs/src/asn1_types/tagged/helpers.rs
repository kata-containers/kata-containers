use super::{Explicit, Implicit, TaggedParser};
use crate::{Any, Error, FromDer, Header, ParseResult, Tag, Tagged};
use nom::error::ParseError;
use nom::{Err, IResult};

// helper functions for parsing tagged objects

pub fn parse_der_tagged_explicit<'a, IntoTag, T, E>(
    tag: IntoTag,
) -> impl FnMut(&'a [u8]) -> ParseResult<TaggedParser<'a, Explicit, T, E>, E>
where
    IntoTag: Into<Tag>,
    TaggedParser<'a, Explicit, T, E>: FromDer<'a, E>,
    E: From<Error>,
{
    let tag = tag.into();
    move |i| {
        let (rem, tagged) = TaggedParser::from_der(i)?;
        tagged.assert_tag(tag).map_err(|e| Err::Error(e.into()))?;
        Ok((rem, tagged))
    }
}

pub fn parse_der_tagged_explicit_g<'a, IntoTag, T, F, E>(
    tag: IntoTag,
    f: F,
) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], T, E>
where
    F: Fn(&'a [u8], Header<'a>) -> IResult<&'a [u8], T, E>,
    E: ParseError<&'a [u8]> + From<Error>,
    IntoTag: Into<Tag>,
{
    let tag = tag.into();
    parse_der_container(tag, move |any: Any<'a>| {
        any.header
            .assert_tag(tag)
            .map_err(|e| nom::Err::convert(e.into()))?;
        f(any.data, any.header)
    })
}

pub fn parse_der_tagged_implicit<'a, IntoTag, T, E>(
    tag: IntoTag,
) -> impl FnMut(&'a [u8]) -> ParseResult<TaggedParser<'a, Implicit, T, E>, E>
where
    IntoTag: Into<Tag>,
    // T: TryFrom<Any<'a>, Error = Error> + Tagged,
    TaggedParser<'a, Implicit, T, E>: FromDer<'a, E>,
    E: From<Error>,
{
    let tag = tag.into();
    move |i| {
        let (rem, tagged) = TaggedParser::from_der(i)?;
        tagged
            .assert_tag(tag)
            .map_err(|e| nom::Err::convert(e.into()))?;
        Ok((rem, tagged))
    }
}

pub fn parse_der_tagged_implicit_g<'a, IntoTag, T, F, E>(
    tag: IntoTag,
    f: F,
) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], T, E>
where
    F: Fn(&'a [u8], Tag, Header<'a>) -> IResult<&'a [u8], T, E>,
    E: ParseError<&'a [u8]> + From<Error>,
    IntoTag: Into<Tag>,
    T: Tagged,
{
    let tag = tag.into();
    parse_der_container(tag, move |any: Any<'a>| {
        // verify tag of external header
        any.header
            .assert_tag(tag)
            .map_err(|e| nom::Err::convert(e.into()))?;
        // build a fake header with the expected tag
        let Any { header, data } = any;
        let header = Header {
            tag: T::TAG,
            ..header.clone()
        };
        f(data, tag, header)
    })
}

fn parse_der_container<'a, T, F, E>(
    tag: Tag,
    f: F,
) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], T, E>
where
    F: Fn(Any<'a>) -> IResult<&'a [u8], T, E>,
    E: ParseError<&'a [u8]> + From<Error>,
{
    move |i: &[u8]| {
        let (rem, any) = Any::from_der(i).map_err(nom::Err::convert)?;
        any.header
            .assert_tag(tag)
            .map_err(|e| nom::Err::convert(e.into()))?;
        let (_, output) = f(any)?;
        Ok((rem, output))
    }
}

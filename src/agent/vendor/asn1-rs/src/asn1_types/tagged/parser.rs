use crate::*;
use core::marker::PhantomData;

#[derive(Debug, PartialEq)]
pub struct TaggedParser<'a, TagKind, T, E = Error> {
    pub header: Header<'a>,
    pub inner: T,

    pub(crate) tag_kind: PhantomData<TagKind>,
    pub(crate) _e: PhantomData<E>,
}

impl<'a, TagKind, T, E> TaggedParser<'a, TagKind, T, E> {
    pub const fn new(header: Header<'a>, inner: T) -> Self {
        TaggedParser {
            header,
            inner,
            tag_kind: PhantomData,
            _e: PhantomData,
        }
    }

    pub const fn assert_class(&self, class: Class) -> Result<()> {
        self.header.assert_class(class)
    }

    pub const fn assert_tag(&self, tag: Tag) -> Result<()> {
        self.header.assert_tag(tag)
    }

    #[inline]
    pub const fn class(&self) -> Class {
        self.header.class
    }

    #[inline]
    pub const fn tag(&self) -> Tag {
        self.header.tag
    }
}

impl<'a, TagKind, T, E> AsRef<T> for TaggedParser<'a, TagKind, T, E> {
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

impl<'a, TagKind, T, E> TaggedParser<'a, TagKind, T, E>
where
    Self: FromBer<'a, E>,
    E: From<Error>,
{
    pub fn parse_ber(class: Class, tag: Tag, bytes: &'a [u8]) -> ParseResult<'a, Self, E> {
        let (rem, t) = TaggedParser::<TagKind, T, E>::from_ber(bytes)?;
        t.assert_class(class).map_err(|e| Err::Error(e.into()))?;
        t.assert_tag(tag).map_err(|e| Err::Error(e.into()))?;
        Ok((rem, t))
    }
}

impl<'a, TagKind, T, E> TaggedParser<'a, TagKind, T, E>
where
    Self: FromDer<'a, E>,
    E: From<Error>,
{
    pub fn parse_der(class: Class, tag: Tag, bytes: &'a [u8]) -> ParseResult<'a, Self, E> {
        let (rem, t) = TaggedParser::<TagKind, T, E>::from_der(bytes)?;
        t.assert_class(class).map_err(|e| Err::Error(e.into()))?;
        t.assert_tag(tag).map_err(|e| Err::Error(e.into()))?;
        Ok((rem, t))
    }
}

impl<'a, TagKind, T, E> DynTagged for TaggedParser<'a, TagKind, T, E> {
    fn tag(&self) -> Tag {
        self.tag()
    }
}

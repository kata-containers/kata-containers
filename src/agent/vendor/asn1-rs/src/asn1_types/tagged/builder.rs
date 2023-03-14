use super::{Error, Explicit, Implicit, TaggedParser};
use crate::{Class, FromBer, FromDer, ParseResult, Tag};
use core::marker::PhantomData;

/// A builder for parsing tagged values (`IMPLICIT` or `EXPLICIT`)
///
/// # Examples
///
/// ```
/// use asn1_rs::{Class, Tag, TaggedParserBuilder};
///
/// let parser = TaggedParserBuilder::explicit()
///     .with_class(Class::ContextSpecific)
///     .with_tag(Tag(0))
///     .der_parser::<u32>();
///
/// let input = &[0xa0, 0x03, 0x02, 0x01, 0x02];
/// let (rem, tagged) = parser(input).expect("parsing failed");
///
/// assert!(rem.is_empty());
/// assert_eq!(tagged.tag(), Tag(0));
/// assert_eq!(tagged.as_ref(), &2);
/// ```
#[derive(Clone, Copy, Debug)]
pub struct TaggedParserBuilder<TagKind, E = Error> {
    class: Class,
    tag: Tag,
    tag_kind: PhantomData<TagKind>,
    _e: PhantomData<E>,
}

impl<TagKind, E> TaggedParserBuilder<TagKind, E> {
    /// Create a default `TaggedParserBuilder` builder
    ///
    /// `TagKind` must be specified as either [`Explicit`] or [`Implicit`]
    ///
    /// ```
    /// use asn1_rs::{Explicit, TaggedParserBuilder};
    ///
    /// let builder = TaggedParserBuilder::<Explicit>::new();
    /// ```
    pub const fn new() -> Self {
        TaggedParserBuilder {
            class: Class::Universal,
            tag: Tag(0),
            tag_kind: PhantomData,
            _e: PhantomData,
        }
    }

    /// Set the expected `Class` for the builder
    pub const fn with_class(self, class: Class) -> Self {
        Self { class, ..self }
    }

    /// Set the expected `Tag` for the builder
    pub const fn with_tag(self, tag: Tag) -> Self {
        Self { tag, ..self }
    }
}

impl<E> TaggedParserBuilder<Explicit, E> {
    /// Create a `TagParser` builder for `EXPLICIT` tagged values
    pub const fn explicit() -> Self {
        TaggedParserBuilder::new()
    }
}

impl<E> TaggedParserBuilder<Implicit, E> {
    /// Create a `TagParser` builder for `IMPLICIT` tagged values
    pub const fn implicit() -> Self {
        TaggedParserBuilder::new()
    }
}

impl<TagKind, E> TaggedParserBuilder<TagKind, E> {
    /// Create the BER parser from the builder parameters
    ///
    /// This method will consume the builder and return a parser (to be used as a function).
    pub fn ber_parser<'a, T>(
        self,
    ) -> impl Fn(&'a [u8]) -> ParseResult<'a, TaggedParser<'a, TagKind, T, E>, E>
    where
        TaggedParser<'a, TagKind, T, E>: FromBer<'a, E>,
        E: From<Error>,
    {
        move |bytes: &[u8]| TaggedParser::<TagKind, T, E>::parse_ber(self.class, self.tag, bytes)
    }
}

impl<TagKind, E> TaggedParserBuilder<TagKind, E> {
    /// Create the DER parser from the builder parameters
    ///
    /// This method will consume the builder and return a parser (to be used as a function).
    pub fn der_parser<'a, T>(
        self,
    ) -> impl Fn(&'a [u8]) -> ParseResult<'a, TaggedParser<'a, TagKind, T, E>, E>
    where
        TaggedParser<'a, TagKind, T, E>: FromDer<'a, E>,
        E: From<Error>,
    {
        move |bytes: &[u8]| TaggedParser::<TagKind, T, E>::parse_der(self.class, self.tag, bytes)
    }
}

use super::{explicit::TaggedExplicit, implicit::TaggedImplicit};
use crate::*;

/// Helper object to parse TAGGED OPTIONAL types (explicit or implicit)
///
/// This object can be used similarly to a builder pattern, to specify the expected class and
/// tag of the object to parse, and the content parsing function.
///
/// The content parsing function takes two arguments: the outer header, and the data.
///
/// It can be used for both EXPLICIT or IMPLICIT tagged objects by using parsing functions that
/// expect a header (or not) in the contents.
///
/// The [`OptTaggedParser::from`] method is a shortcut to build an object with `ContextSpecific`
/// class and the given tag. The [`OptTaggedParser::new`] method is more generic.
///
/// See also [`OptTaggedExplicit`] and [`OptTaggedImplicit`] for alternatives that implement [`FromBer`]/
/// [`FromDer`].
///
/// # Examples
///
/// To parse a `[APPLICATION 0] EXPLICIT INTEGER OPTIONAL` object:
///
/// ```rust
/// use asn1_rs::{Class, FromDer, Integer, Tag, OptTaggedParser};
///
/// let bytes = &[0x60, 0x03, 0x2, 0x1, 0x2];
///
/// let (_, tagged) = OptTaggedParser::new(Class::Application, Tag(0))
///                     .parse_der(bytes, |_, data| Integer::from_der(data))
///                     .unwrap();
///
/// assert_eq!(tagged, Some(Integer::from(2)));
/// ```
///
/// To parse a `[0] IMPLICIT INTEGER OPTIONAL` object:
///
/// ```rust
/// use asn1_rs::{Error, Integer, OptTaggedParser};
///
/// let bytes = &[0xa0, 0x1, 0x2];
///
/// let (_, tagged) = OptTaggedParser::from(0)
///                     .parse_der::<_, Error, _>(bytes, |_, data| Ok((&[], Integer::new(data))))
///                     .unwrap();
///
/// assert_eq!(tagged, Some(Integer::from(2)));
/// ```
#[derive(Debug)]
pub struct OptTaggedParser {
    /// The expected class for the object to parse
    pub class: Class,
    /// The expected tag for the object to parse
    pub tag: Tag,
}

impl OptTaggedParser {
    /// Build a new `OptTaggedParser` object.
    ///
    /// If using `Class::ContextSpecific`, using [`OptTaggedParser::from`] with either a `Tag` or `u32` is
    /// a shorter way to build this object.
    pub const fn new(class: Class, tag: Tag) -> Self {
        OptTaggedParser { class, tag }
    }

    pub const fn universal(tag: u32) -> Self {
        Self::new(Class::Universal, Tag(tag))
    }

    pub const fn tagged(tag: u32) -> Self {
        Self::new(Class::ContextSpecific, Tag(tag))
    }

    pub const fn application(tag: u32) -> Self {
        Self::new(Class::Application, Tag(tag))
    }

    pub const fn private(tag: u32) -> Self {
        Self::new(Class::Private, Tag(tag))
    }

    /// Parse input as BER, and apply the provided function to parse object.
    ///
    /// Returns the remaining bytes, and `Some(T)` if expected tag was found, else `None`.
    ///
    ///  This function returns an error if tag was found but has a different class, or if parsing fails.
    ///
    /// # Examples
    ///
    /// To parse a `[0] EXPLICIT INTEGER OPTIONAL` object:
    ///
    /// ```rust
    /// use asn1_rs::{FromBer, Integer, OptTaggedParser};
    ///
    /// let bytes = &[0xa0, 0x03, 0x2, 0x1, 0x2];
    ///
    /// let (_, tagged) = OptTaggedParser::from(0)
    ///                     .parse_ber(bytes, |_, data| Integer::from_ber(data))
    ///                     .unwrap();
    ///
    /// assert_eq!(tagged, Some(Integer::from(2)));
    /// ```
    pub fn parse_ber<'a, T, E, F>(&self, bytes: &'a [u8], f: F) -> ParseResult<'a, Option<T>, E>
    where
        F: Fn(Header, &'a [u8]) -> ParseResult<'a, T, E>,
        E: From<Error>,
    {
        if bytes.is_empty() {
            return Ok((bytes, None));
        }
        let (rem, any) = Any::from_ber(bytes).map_err(Err::convert)?;
        if any.tag() != self.tag {
            return Ok((bytes, None));
        }
        if any.class() != self.class {
            return Err(Err::Error(
                Error::unexpected_class(Some(self.class), any.class()).into(),
            ));
        }
        let Any { header, data } = any;
        let (_, res) = f(header, data)?;
        Ok((rem, Some(res)))
    }

    /// Parse input as DER, and apply the provided function to parse object.
    ///
    /// Returns the remaining bytes, and `Some(T)` if expected tag was found, else `None`.
    ///
    ///  This function returns an error if tag was found but has a different class, or if parsing fails.
    ///
    /// # Examples
    ///
    /// To parse a `[0] EXPLICIT INTEGER OPTIONAL` object:
    ///
    /// ```rust
    /// use asn1_rs::{FromDer, Integer, OptTaggedParser};
    ///
    /// let bytes = &[0xa0, 0x03, 0x2, 0x1, 0x2];
    ///
    /// let (_, tagged) = OptTaggedParser::from(0)
    ///                     .parse_der(bytes, |_, data| Integer::from_der(data))
    ///                     .unwrap();
    ///
    /// assert_eq!(tagged, Some(Integer::from(2)));
    /// ```
    pub fn parse_der<'a, T, E, F>(&self, bytes: &'a [u8], f: F) -> ParseResult<'a, Option<T>, E>
    where
        F: Fn(Header, &'a [u8]) -> ParseResult<'a, T, E>,
        E: From<Error>,
    {
        if bytes.is_empty() {
            return Ok((bytes, None));
        }
        let (rem, any) = Any::from_der(bytes).map_err(Err::convert)?;
        if any.tag() != self.tag {
            return Ok((bytes, None));
        }
        if any.class() != self.class {
            return Err(Err::Error(
                Error::unexpected_class(Some(self.class), any.class()).into(),
            ));
        }
        let Any { header, data } = any;
        let (_, res) = f(header, data)?;
        Ok((rem, Some(res)))
    }
}

impl From<Tag> for OptTaggedParser {
    /// Build a `TaggedOptional` object with class `ContextSpecific` and given tag
    #[inline]
    fn from(tag: Tag) -> Self {
        OptTaggedParser::new(Class::ContextSpecific, tag)
    }
}

impl From<u32> for OptTaggedParser {
    /// Build a `TaggedOptional` object with class `ContextSpecific` and given tag
    #[inline]
    fn from(tag: u32) -> Self {
        OptTaggedParser::new(Class::ContextSpecific, Tag(tag))
    }
}

/// A helper object to parse `[ n ] EXPLICIT T OPTIONAL`
///
/// A helper object implementing [`FromBer`] and [`FromDer`], to parse tagged
/// optional values.
///
/// This helper expects context-specific tags.
/// Use `Option<` [`TaggedValue`] `>` for a more generic implementation.
///
/// # Examples
///
/// To parse a `[0] EXPLICIT INTEGER OPTIONAL` object:
///
/// ```rust
/// use asn1_rs::{Error, FromBer, Integer, OptTaggedExplicit, TaggedValue};
///
/// let bytes = &[0xa0, 0x03, 0x2, 0x1, 0x2];
///
/// // If tagged object is present (and has expected tag), parsing succeeds:
/// let (_, tagged) = OptTaggedExplicit::<Integer, Error, 0>::from_ber(bytes).unwrap();
/// assert_eq!(tagged, Some(TaggedValue::explicit(Integer::from(2))));
///
/// // If tagged object is not present or has different tag, parsing
/// // also succeeds (returning None):
/// let (_, tagged) = OptTaggedExplicit::<Integer, Error, 0>::from_ber(&[]).unwrap();
/// assert_eq!(tagged, None);
/// ```
pub type OptTaggedExplicit<T, E, const TAG: u32> = Option<TaggedExplicit<T, E, TAG>>;

/// A helper object to parse `[ n ] IMPLICIT T OPTIONAL`
///
/// A helper object implementing [`FromBer`] and [`FromDer`], to parse tagged
/// optional values.
///
/// This helper expects context-specific tags.
/// Use `Option<` [`TaggedValue`] `>` for a more generic implementation.
///
/// # Examples
///
/// To parse a `[0] IMPLICIT INTEGER OPTIONAL` object:
///
/// ```rust
/// use asn1_rs::{Error, FromBer, Integer, OptTaggedImplicit, TaggedValue};
///
/// let bytes = &[0xa0, 0x1, 0x2];
///
/// let (_, tagged) = OptTaggedImplicit::<Integer, Error, 0>::from_ber(bytes).unwrap();
/// assert_eq!(tagged, Some(TaggedValue::implicit(Integer::from(2))));
///
/// // If tagged object is not present or has different tag, parsing
/// // also succeeds (returning None):
/// let (_, tagged) = OptTaggedImplicit::<Integer, Error, 0>::from_ber(&[]).unwrap();
/// assert_eq!(tagged, None);
/// ```
pub type OptTaggedImplicit<T, E, const TAG: u32> = Option<TaggedImplicit<T, E, TAG>>;

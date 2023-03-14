use crate::*;
use core::convert::TryFrom;
use core::marker::PhantomData;

impl<'a, T, E, const CLASS: u8, const TAG: u32> TryFrom<Any<'a>>
    for TaggedValue<T, E, Explicit, CLASS, TAG>
where
    T: FromBer<'a, E>,
    E: From<Error>,
{
    type Error = E;

    fn try_from(any: Any<'a>) -> Result<Self, E> {
        Self::try_from(&any)
    }
}

impl<'a, 'b, T, E, const CLASS: u8, const TAG: u32> TryFrom<&'b Any<'a>>
    for TaggedValue<T, E, Explicit, CLASS, TAG>
where
    T: FromBer<'a, E>,
    E: From<Error>,
{
    type Error = E;

    fn try_from(any: &'b Any<'a>) -> Result<Self, E> {
        any.tag().assert_eq(Tag(TAG))?;
        any.header.assert_constructed()?;
        if any.class() as u8 != CLASS {
            let class = Class::try_from(CLASS).ok();
            return Err(Error::unexpected_class(class, any.class()).into());
        }
        let (_, inner) = match T::from_ber(any.data) {
            Ok((rem, res)) => (rem, res),
            Err(Err::Error(e)) | Err(Err::Failure(e)) => return Err(e),
            Err(Err::Incomplete(n)) => return Err(Error::Incomplete(n).into()),
        };
        Ok(TaggedValue::explicit(inner))
    }
}

impl<'a, T, E, const CLASS: u8, const TAG: u32> FromDer<'a, E>
    for TaggedValue<T, E, Explicit, CLASS, TAG>
where
    T: FromDer<'a, E>,
    E: From<Error>,
{
    fn from_der(bytes: &'a [u8]) -> ParseResult<'a, Self, E> {
        let (rem, any) = Any::from_der(bytes).map_err(Err::convert)?;
        any.tag()
            .assert_eq(Tag(TAG))
            .map_err(|e| Err::Error(e.into()))?;
        any.header
            .assert_constructed()
            .map_err(|e| Err::Error(e.into()))?;
        if any.class() as u8 != CLASS {
            let class = Class::try_from(CLASS).ok();
            return Err(Err::Error(
                Error::unexpected_class(class, any.class()).into(),
            ));
        }
        let (_, inner) = T::from_der(any.data)?;
        Ok((rem, TaggedValue::explicit(inner)))
    }
}

impl<'a, T, E, const CLASS: u8, const TAG: u32> CheckDerConstraints
    for TaggedValue<T, E, Explicit, CLASS, TAG>
where
    T: CheckDerConstraints,
{
    fn check_constraints(any: &Any) -> Result<()> {
        any.header.length.assert_definite()?;
        let (_, inner) = Any::from_ber(any.data)?;
        T::check_constraints(&inner)?;
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<T, E, const CLASS: u8, const TAG: u32> ToDer for TaggedValue<T, E, Explicit, CLASS, TAG>
where
    T: ToDer,
{
    fn to_der_len(&self) -> Result<usize> {
        let sz = self.inner.to_der_len()?;
        if sz < 127 {
            // 1 (class+tag) + 1 (length) + len
            Ok(2 + sz)
        } else {
            // 1 (class+tag) + n (length) + len
            let n = Length::Definite(sz).to_der_len()?;
            Ok(1 + n + sz)
        }
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let inner_len = self.inner.to_der_len()?;
        let class =
            Class::try_from(CLASS).map_err(|_| SerializeError::InvalidClass { class: CLASS })?;
        let header = Header::new(class, true, self.tag(), Length::Definite(inner_len));
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        self.inner.write_der(writer)
    }
}

/// A helper object to parse `[ n ] EXPLICIT T`
///
/// A helper object implementing [`FromBer`] and [`FromDer`], to parse tagged
/// optional values.
///
/// This helper expects context-specific tags.
/// See [`TaggedValue`] or [`TaggedParser`] for more generic implementations if needed.
///
/// # Examples
///
/// To parse a `[0] EXPLICIT INTEGER` object:
///
/// ```rust
/// use asn1_rs::{Error, FromBer, Integer, TaggedExplicit, TaggedValue};
///
/// let bytes = &[0xa0, 0x03, 0x2, 0x1, 0x2];
///
/// // If tagged object is present (and has expected tag), parsing succeeds:
/// let (_, tagged) = TaggedExplicit::<Integer, Error, 0>::from_ber(bytes).unwrap();
/// assert_eq!(tagged, TaggedValue::explicit(Integer::from(2)));
/// ```
pub type TaggedExplicit<T, E, const TAG: u32> = TaggedValue<T, E, Explicit, CONTEXT_SPECIFIC, TAG>;

// implementations for TaggedParser

impl<'a, T, E> TaggedParser<'a, Explicit, T, E> {
    pub const fn new_explicit(class: Class, tag: u32, inner: T) -> Self {
        Self {
            header: Header::new(class, true, Tag(tag), Length::Definite(0)),
            inner,
            tag_kind: PhantomData,
            _e: PhantomData,
        }
    }

    /// Parse a BER tagged value and apply the provided parsing function to content
    ///
    /// After parsing, the sequence object and header are discarded.
    ///
    /// Note: this function is provided for `Explicit`, but there is not difference between
    /// explicit or implicit tags. The `op` function is responsible of handling the content.
    #[inline]
    pub fn from_ber_and_then<F>(
        class: Class,
        tag: u32,
        bytes: &'a [u8],
        op: F,
    ) -> ParseResult<'a, T, E>
    where
        F: FnOnce(&'a [u8]) -> ParseResult<T, E>,
        E: From<Error>,
    {
        Any::from_ber_and_then(class, tag, bytes, op)
    }

    /// Parse a DER tagged value and apply the provided parsing function to content
    ///
    /// After parsing, the sequence object and header are discarded.
    ///
    /// Note: this function is provided for `Explicit`, but there is not difference between
    /// explicit or implicit tags. The `op` function is responsible of handling the content.
    #[inline]
    pub fn from_der_and_then<F>(
        class: Class,
        tag: u32,
        bytes: &'a [u8],
        op: F,
    ) -> ParseResult<'a, T, E>
    where
        F: FnOnce(&'a [u8]) -> ParseResult<T, E>,
        E: From<Error>,
    {
        Any::from_der_and_then(class, tag, bytes, op)
    }
}

impl<'a, T, E> FromBer<'a, E> for TaggedParser<'a, Explicit, T, E>
where
    T: FromBer<'a, E>,
    E: From<Error>,
{
    fn from_ber(bytes: &'a [u8]) -> ParseResult<'a, Self, E> {
        let (rem, any) = Any::from_ber(bytes).map_err(Err::convert)?;
        let header = any.header;
        let (_, inner) = T::from_ber(any.data)?;
        let tagged = TaggedParser {
            header,
            inner,
            tag_kind: PhantomData,
            _e: PhantomData,
        };
        Ok((rem, tagged))
    }
}

impl<'a, T, E> FromDer<'a, E> for TaggedParser<'a, Explicit, T, E>
where
    T: FromDer<'a, E>,
    E: From<Error>,
{
    fn from_der(bytes: &'a [u8]) -> ParseResult<'a, Self, E> {
        let (rem, any) = Any::from_der(bytes).map_err(Err::convert)?;
        let header = any.header;
        let (_, inner) = T::from_der(any.data)?;
        let tagged = TaggedParser {
            header,
            inner,
            tag_kind: PhantomData,
            _e: PhantomData,
        };
        Ok((rem, tagged))
    }
}

impl<'a, T> CheckDerConstraints for TaggedParser<'a, Explicit, T>
where
    T: CheckDerConstraints,
{
    fn check_constraints(any: &Any) -> Result<()> {
        any.header.length.assert_definite()?;
        let (_, inner_any) = Any::from_der(any.data)?;
        T::check_constraints(&inner_any)?;
        Ok(())
    }
}

#[cfg(feature = "std")]
impl<'a, T> ToDer for TaggedParser<'a, Explicit, T>
where
    T: ToDer,
{
    fn to_der_len(&self) -> Result<usize> {
        let sz = self.inner.to_der_len()?;
        if sz < 127 {
            // 1 (class+tag) + 1 (length) + len
            Ok(2 + sz)
        } else {
            // 1 (class+tag) + n (length) + len
            let n = Length::Definite(sz).to_der_len()?;
            Ok(1 + n + sz)
        }
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let inner_len = self.inner.to_der_len()?;
        let header = Header::new(self.class(), true, self.tag(), Length::Definite(inner_len));
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        self.inner.write_der(writer)
    }
}

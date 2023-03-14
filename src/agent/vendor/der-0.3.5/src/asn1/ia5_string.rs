//! ASN.1 `IA5String` support.

use crate::{
    str_slice::StrSlice, Any, Encodable, Encoder, Error, ErrorKind, Length, Result, Tag, Tagged,
};
use core::{convert::TryFrom, fmt, str};

/// ASN.1 `IA5String` type.
///
/// Supports the [International Alphabet No. 5 (IA5)] character encoding, i.e.
/// the lower 128 characters of the ASCII alphabet. (Note: IA5 is now
/// technically known as the International Reference Alphabet or IRA as
/// specified in the ITU-T's T.50 recommendation).
///
/// For UTF-8, use [`Utf8String`][`crate::Utf8String`].
///
/// [International Alphabet No. 5 (IA5)]: https://en.wikipedia.org/wiki/T.50_%28standard%29
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Ia5String<'a> {
    /// Inner value
    inner: StrSlice<'a>,
}

impl<'a> Ia5String<'a> {
    /// Create a new `IA5String`.
    pub fn new<T>(input: &'a T) -> Result<Self>
    where
        T: AsRef<[u8]> + ?Sized,
    {
        let input = input.as_ref();

        // Validate all characters are within IA5String's allowed set
        if input.iter().any(|&c| c > 0x7F) {
            return Err(ErrorKind::Value { tag: Self::TAG }.into());
        }

        StrSlice::from_bytes(input)
            .map(|inner| Self { inner })
            .map_err(|_| ErrorKind::Value { tag: Self::TAG }.into())
    }

    /// Borrow the string as a `str`.
    pub fn as_str(&self) -> &'a str {
        self.inner.as_str()
    }

    /// Borrow the string as bytes.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.inner.as_bytes()
    }

    /// Get the length of the inner byte slice.
    pub fn len(&self) -> Length {
        self.inner.len()
    }

    /// Is the inner string empty?
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl AsRef<str> for Ia5String<'_> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for Ia5String<'_> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<'a> From<&Ia5String<'a>> for Ia5String<'a> {
    fn from(value: &Ia5String<'a>) -> Ia5String<'a> {
        *value
    }
}

impl<'a> TryFrom<Any<'a>> for Ia5String<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Ia5String<'a>> {
        any.tag().assert_eq(Tag::Ia5String)?;
        Self::new(any.as_bytes())
    }
}

impl<'a> From<Ia5String<'a>> for Any<'a> {
    fn from(printable_string: Ia5String<'a>) -> Any<'a> {
        Any::from_tag_and_value(Tag::Ia5String, printable_string.inner.into())
    }
}

impl<'a> From<Ia5String<'a>> for &'a [u8] {
    fn from(printable_string: Ia5String<'a>) -> &'a [u8] {
        printable_string.as_bytes()
    }
}

impl<'a> Encodable for Ia5String<'a> {
    fn encoded_len(&self) -> Result<Length> {
        Any::from(*self).encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Any::from(*self).encode(encoder)
    }
}

impl<'a> Tagged for Ia5String<'a> {
    const TAG: Tag = Tag::Ia5String;
}

impl<'a> fmt::Display for Ia5String<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'a> fmt::Debug for Ia5String<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Ia5String({:?})", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::Ia5String;
    use crate::Decodable;
    use hex_literal::hex;

    #[test]
    fn parse_bytes() {
        let example_bytes = hex!("16 0d 74 65 73 74 31 40 72 73 61 2e 63 6f 6d");
        let printable_string = Ia5String::from_der(&example_bytes).unwrap();
        assert_eq!(printable_string.as_str(), "test1@rsa.com");
    }
}

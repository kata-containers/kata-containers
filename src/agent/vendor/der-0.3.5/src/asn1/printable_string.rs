//! ASN.1 `PrintableString` support.

use crate::{
    str_slice::StrSlice, Any, Encodable, Encoder, Error, ErrorKind, Length, Result, Tag, Tagged,
};
use core::{convert::TryFrom, fmt, str};

/// ASN.1 `PrintableString` type.
///
/// Supports a subset the ASCII character set (desribed below).
///
/// For UTF-8, use [`Utf8String`][`crate::Utf8String`] instead. For the full
/// ASCII character set, use [`Ia5String`][`crate::Ia5String`].
///
/// # Supported characters
///
/// The following ASCII characters/ranges are supported:
///
/// - `A..Z`
/// - `a..z`
/// - `0..9`
/// - "` `" (i.e. space)
/// - `\`
/// - `(`
/// - `)`
/// - `+`
/// - `,`
/// - `-`
/// - `.`
/// - `/`
/// - `:`
/// - `=`
/// - `?`
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct PrintableString<'a> {
    /// Inner value
    inner: StrSlice<'a>,
}

impl<'a> PrintableString<'a> {
    /// Create a new ASN.1 `PrintableString`.
    pub fn new<T>(input: &'a T) -> Result<Self>
    where
        T: AsRef<[u8]> + ?Sized,
    {
        let input = input.as_ref();

        // Validate all characters are within PrintedString's allowed set
        for &c in input.iter() {
            match c {
                b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b' '
                | b'\''
                | b'('
                | b')'
                | b'+'
                | b','
                | b'-'
                | b'.'
                | b'/'
                | b':'
                | b'='
                | b'?' => (),
                _ => return Err(ErrorKind::Value { tag: Self::TAG }.into()),
            }
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

impl AsRef<str> for PrintableString<'_> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for PrintableString<'_> {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<'a> From<&PrintableString<'a>> for PrintableString<'a> {
    fn from(value: &PrintableString<'a>) -> PrintableString<'a> {
        *value
    }
}

impl<'a> TryFrom<Any<'a>> for PrintableString<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<PrintableString<'a>> {
        any.tag().assert_eq(Tag::PrintableString)?;
        Self::new(any.as_bytes())
    }
}

impl<'a> From<PrintableString<'a>> for Any<'a> {
    fn from(printable_string: PrintableString<'a>) -> Any<'a> {
        Any::from_tag_and_value(Tag::PrintableString, printable_string.inner.into())
    }
}

impl<'a> From<PrintableString<'a>> for &'a [u8] {
    fn from(printable_string: PrintableString<'a>) -> &'a [u8] {
        printable_string.as_bytes()
    }
}

impl<'a> Encodable for PrintableString<'a> {
    fn encoded_len(&self) -> Result<Length> {
        Any::from(*self).encoded_len()
    }

    fn encode(&self, encoder: &mut Encoder<'_>) -> Result<()> {
        Any::from(*self).encode(encoder)
    }
}

impl<'a> Tagged for PrintableString<'a> {
    const TAG: Tag = Tag::PrintableString;
}

impl<'a> fmt::Display for PrintableString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<'a> fmt::Debug for PrintableString<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrintableString({:?})", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::PrintableString;
    use crate::Decodable;

    #[test]
    fn parse_bytes() {
        let example_bytes = &[
            0x13, 0x0b, 0x54, 0x65, 0x73, 0x74, 0x20, 0x55, 0x73, 0x65, 0x72, 0x20, 0x31,
        ];

        let printable_string = PrintableString::from_der(example_bytes).unwrap();
        assert_eq!(printable_string.as_str(), "Test User 1");
    }
}

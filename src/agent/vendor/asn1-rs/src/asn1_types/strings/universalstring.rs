// do not use the `asn1_string` macro, since types are not the same
// X.680 section 37.6 and X.690 section 8.21.7

use crate::*;
use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::iter::FromIterator;

/// ASN.1 `UniversalString` type
///
/// Note: parsing a `UniversalString` allocates memory since the UCS-4 to UTF-8 conversion requires a memory allocation.
#[derive(Debug, PartialEq)]
pub struct UniversalString<'a> {
    pub(crate) data: Cow<'a, str>,
}

impl<'a> UniversalString<'a> {
    pub const fn new(s: &'a str) -> Self {
        UniversalString {
            data: Cow::Borrowed(s),
        }
    }

    pub fn string(&self) -> String {
        self.data.to_string()
    }
}

impl<'a> AsRef<str> for UniversalString<'a> {
    fn as_ref(&self) -> &str {
        &self.data
    }
}

impl<'a> From<&'a str> for UniversalString<'a> {
    fn from(s: &'a str) -> Self {
        Self::new(s)
    }
}

impl From<String> for UniversalString<'_> {
    fn from(s: String) -> Self {
        Self {
            data: alloc::borrow::Cow::Owned(s),
        }
    }
}

impl<'a> TryFrom<Any<'a>> for UniversalString<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<UniversalString<'a>> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for UniversalString<'a> {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<UniversalString<'a>> {
        any.tag().assert_eq(Self::TAG)?;

        if any.data.len() % 4 != 0 {
            return Err(Error::StringInvalidCharset);
        }

        // read slice as big-endian UCS-4 string
        let v = &any
            .data
            .chunks(4)
            .map(|s| match s {
                [a, b, c, d] => {
                    let u32_val = ((*a as u32) << 24)
                        | ((*b as u32) << 16)
                        | ((*c as u32) << 8)
                        | (*d as u32);
                    char::from_u32(u32_val)
                }
                _ => unreachable!(),
            })
            .collect::<Option<Vec<_>>>()
            .ok_or(Error::StringInvalidCharset)?;

        let s = String::from_iter(v);
        let data = Cow::Owned(s);

        Ok(UniversalString { data })
    }
}

impl<'a> CheckDerConstraints for UniversalString<'a> {
    fn check_constraints(any: &Any) -> Result<()> {
        any.header.assert_primitive()?;
        Ok(())
    }
}

impl DerAutoDerive for UniversalString<'_> {}

impl<'a> Tagged for UniversalString<'a> {
    const TAG: Tag = Tag::UniversalString;
}

#[cfg(feature = "std")]
impl ToDer for UniversalString<'_> {
    fn to_der_len(&self) -> Result<usize> {
        // UCS-4: 4 bytes per character
        let sz = self.data.as_bytes().len() * 4;
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
        let header = Header::new(
            Class::Universal,
            false,
            Self::TAG,
            Length::Definite(self.data.as_bytes().len() * 4),
        );
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        self.data
            .chars()
            .try_for_each(|c| writer.write(&(c as u32).to_be_bytes()[..]).map(|_| ()))?;
        Ok(self.data.as_bytes().len() * 4)
    }
}

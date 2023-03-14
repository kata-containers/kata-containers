use crate::*;
use alloc::borrow::Cow;
use core::convert::TryFrom;

/// ASN.1 `OCTETSTRING` type
#[derive(Debug, PartialEq, Eq)]
pub struct OctetString<'a> {
    data: Cow<'a, [u8]>,
}

impl<'a> OctetString<'a> {
    pub const fn new(s: &'a [u8]) -> Self {
        OctetString {
            data: Cow::Borrowed(s),
        }
    }

    /// Get the bytes representation of the *content*
    pub fn as_cow(&'a self) -> &Cow<'a, [u8]> {
        &self.data
    }

    /// Get the bytes representation of the *content*
    pub fn into_cow(self) -> Cow<'a, [u8]> {
        self.data
    }
}

impl<'a> AsRef<[u8]> for OctetString<'a> {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl<'a> From<&'a [u8]> for OctetString<'a> {
    fn from(b: &'a [u8]) -> Self {
        OctetString {
            data: Cow::Borrowed(b),
        }
    }
}

impl<'a> TryFrom<Any<'a>> for OctetString<'a> {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<OctetString<'a>> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for OctetString<'a> {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<OctetString<'a>> {
        any.tag().assert_eq(Self::TAG)?;
        Ok(OctetString {
            data: Cow::Borrowed(any.data),
        })
    }
}

impl<'a> CheckDerConstraints for OctetString<'a> {
    fn check_constraints(any: &Any) -> Result<()> {
        // X.690 section 10.2
        any.header.assert_primitive()?;
        Ok(())
    }
}

impl DerAutoDerive for OctetString<'_> {}

impl<'a> Tagged for OctetString<'a> {
    const TAG: Tag = Tag::OctetString;
}

#[cfg(feature = "std")]
impl ToDer for OctetString<'_> {
    fn to_der_len(&self) -> Result<usize> {
        let sz = self.data.len();
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
            Length::Definite(self.data.len()),
        );
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(&self.data).map_err(Into::into)
    }
}

impl<'a> TryFrom<Any<'a>> for &'a [u8] {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<&'a [u8]> {
        any.tag().assert_eq(Self::TAG)?;
        let s = OctetString::try_from(any)?;
        match s.data {
            Cow::Borrowed(s) => Ok(s),
            Cow::Owned(_) => Err(Error::LifetimeError),
        }
    }
}

impl<'a> CheckDerConstraints for &'a [u8] {
    fn check_constraints(any: &Any) -> Result<()> {
        // X.690 section 10.2
        any.header.assert_primitive()?;
        Ok(())
    }
}

impl DerAutoDerive for &'_ [u8] {}

impl<'a> Tagged for &'a [u8] {
    const TAG: Tag = Tag::OctetString;
}

#[cfg(feature = "std")]
impl ToDer for &'_ [u8] {
    fn to_der_len(&self) -> Result<usize> {
        let header = Header::new(
            Class::Universal,
            false,
            Self::TAG,
            Length::Definite(self.len()),
        );
        Ok(header.to_der_len()? + self.len())
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let header = Header::new(
            Class::Universal,
            false,
            Self::TAG,
            Length::Definite(self.len()),
        );
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(self).map_err(Into::into)
    }
}

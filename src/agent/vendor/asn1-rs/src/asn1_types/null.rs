use crate::*;
use core::convert::TryFrom;

/// ASN.1 `NULL` type
#[derive(Debug, PartialEq, Eq)]
pub struct Null {}

impl Null {
    pub const fn new() -> Self {
        Null {}
    }
}

impl<'a> TryFrom<Any<'a>> for Null {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Null> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for Null {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<Null> {
        any.tag().assert_eq(Self::TAG)?;
        if !any.header.length.is_null() {
            return Err(Error::InvalidLength);
        }
        Ok(Null {})
    }
}

impl CheckDerConstraints for Null {
    fn check_constraints(_any: &Any) -> Result<()> {
        Ok(())
    }
}

impl DerAutoDerive for Null {}

impl<'a> Tagged for Null {
    const TAG: Tag = Tag::Null;
}

#[cfg(feature = "std")]
impl ToDer for Null {
    fn to_der_len(&self) -> Result<usize> {
        Ok(2)
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(&[0x05, 0x00]).map_err(Into::into)
    }

    fn write_der_content(&self, _writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        Ok(0)
    }
}

impl<'a> TryFrom<Any<'a>> for () {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<()> {
        any.tag().assert_eq(Self::TAG)?;
        any.header.assert_primitive()?;
        if !any.header.length.is_null() {
            return Err(Error::InvalidLength);
        }
        Ok(())
    }
}

impl<'a> CheckDerConstraints for () {
    fn check_constraints(_any: &Any) -> Result<()> {
        Ok(())
    }
}

impl DerAutoDerive for () {}

impl<'a> Tagged for () {
    const TAG: Tag = Tag::Null;
}

#[cfg(feature = "std")]
impl ToDer for () {
    fn to_der_len(&self) -> Result<usize> {
        Ok(2)
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(&[0x05, 0x00]).map_err(Into::into)
    }

    fn write_der_content(&self, _writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        Ok(0)
    }
}

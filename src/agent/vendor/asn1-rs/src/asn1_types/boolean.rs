use crate::*;
use core::convert::TryFrom;

/// ASN.1 `BOOLEAN` type
///
/// BER objects consider any non-zero value as `true`, and `0` as `false`.
///
/// DER objects must use value `0x0` (`false`) or `0xff` (`true`).
#[derive(Debug, PartialEq)]
pub struct Boolean {
    pub value: u8,
}

impl Boolean {
    /// `BOOLEAN` object for value `false`
    pub const FALSE: Boolean = Boolean::new(0);
    /// `BOOLEAN` object for value `true`
    pub const TRUE: Boolean = Boolean::new(0xff);

    /// Create a new `Boolean` from the provided logical value.
    #[inline]
    pub const fn new(value: u8) -> Self {
        Boolean { value }
    }

    /// Return the `bool` value from this object.
    #[inline]
    pub const fn bool(&self) -> bool {
        self.value != 0
    }
}

impl<'a> TryFrom<Any<'a>> for Boolean {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<Boolean> {
        TryFrom::try_from(&any)
    }
}

// non-consuming version
impl<'a, 'b> TryFrom<&'b Any<'a>> for Boolean {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<Boolean> {
        any.tag().assert_eq(Self::TAG)?;
        // X.690 section 8.2.1:
        // The encoding of a boolean value shall be primitive. The contents octets shall consist of a single octet
        if any.header.length != Length::Definite(1) {
            return Err(Error::InvalidLength);
        }
        let value = any.data[0];
        Ok(Boolean { value })
    }
}

impl<'a> CheckDerConstraints for Boolean {
    fn check_constraints(any: &Any) -> Result<()> {
        let c = any.data[0];
        // X.690 section 11.1
        if !(c == 0 || c == 0xff) {
            return Err(Error::DerConstraintFailed(DerConstraint::InvalidBoolean));
        }
        Ok(())
    }
}

impl DerAutoDerive for Boolean {}

impl<'a> Tagged for Boolean {
    const TAG: Tag = Tag::Boolean;
}

#[cfg(feature = "std")]
impl ToDer for Boolean {
    fn to_der_len(&self) -> Result<usize> {
        // 3 = 1 (tag) + 1 (length) + 1 (value)
        Ok(3)
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(&[Self::TAG.0 as u8, 0x01]).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let b = if self.value != 0 { 0xff } else { 0x00 };
        writer.write(&[b]).map_err(Into::into)
    }

    /// Similar to using `to_der`, but uses header without computing length value
    fn write_der_raw(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let sz = writer.write(&[Self::TAG.0 as u8, 0x01, self.value])?;
        Ok(sz)
    }
}

impl<'a> TryFrom<Any<'a>> for bool {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<bool> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for bool {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<bool> {
        any.tag().assert_eq(Self::TAG)?;
        let b = Boolean::try_from(any)?;
        Ok(b.bool())
    }
}

impl<'a> CheckDerConstraints for bool {
    fn check_constraints(any: &Any) -> Result<()> {
        let c = any.data[0];
        // X.690 section 11.1
        if !(c == 0 || c == 0xff) {
            return Err(Error::DerConstraintFailed(DerConstraint::InvalidBoolean));
        }
        Ok(())
    }
}

impl DerAutoDerive for bool {}

impl<'a> Tagged for bool {
    const TAG: Tag = Tag::Boolean;
}

#[cfg(feature = "std")]
impl ToDer for bool {
    fn to_der_len(&self) -> Result<usize> {
        // 3 = 1 (tag) + 1 (length) + 1 (value)
        Ok(3)
    }

    fn write_der_header(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(&[Self::TAG.0 as u8, 0x01]).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        let b = if *self { 0xff } else { 0x00 };
        writer.write(&[b]).map_err(Into::into)
    }
}

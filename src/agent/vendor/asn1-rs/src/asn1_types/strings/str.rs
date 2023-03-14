use crate::*;
use alloc::borrow::Cow;
use core::convert::TryFrom;

impl<'a> TryFrom<Any<'a>> for &'a str {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<&'a str> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for &'a str {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<&'a str> {
        any.tag().assert_eq(Self::TAG)?;
        let s = Utf8String::try_from(any)?;
        match s.data {
            Cow::Borrowed(s) => Ok(s),
            Cow::Owned(_) => Err(Error::LifetimeError),
        }
    }
}

impl<'a> CheckDerConstraints for &'a str {
    fn check_constraints(any: &Any) -> Result<()> {
        // X.690 section 10.2
        any.header.assert_primitive()?;
        Ok(())
    }
}

impl DerAutoDerive for &'_ str {}

impl<'a> Tagged for &'a str {
    const TAG: Tag = Tag::Utf8String;
}

#[cfg(feature = "std")]
impl<'a> ToDer for &'a str {
    fn to_der_len(&self) -> Result<usize> {
        let sz = self.as_bytes().len();
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
            Length::Definite(self.as_bytes().len()),
        );
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(self.as_bytes()).map_err(Into::into)
    }
}

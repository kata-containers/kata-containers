use crate::*;
use alloc::string::String;
use core::convert::TryFrom;

impl<'a> TryFrom<Any<'a>> for String {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<String> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for String {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<String> {
        any.tag().assert_eq(Self::TAG)?;
        let s = Utf8String::try_from(any)?;
        Ok(s.data.into_owned())
    }
}

impl<'a> CheckDerConstraints for String {
    fn check_constraints(any: &Any) -> Result<()> {
        // X.690 section 10.2
        any.header.assert_primitive()?;
        Ok(())
    }
}

impl DerAutoDerive for String {}

impl Tagged for String {
    const TAG: Tag = Tag::Utf8String;
}

#[cfg(feature = "std")]
impl ToDer for String {
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
            Length::Definite(self.len()),
        );
        header.write_der_header(writer).map_err(Into::into)
    }

    fn write_der_content(&self, writer: &mut dyn std::io::Write) -> SerializeResult<usize> {
        writer.write(self.as_ref()).map_err(Into::into)
    }
}

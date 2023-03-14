use crate::{Any, Error, Result, Tag, Tagged};
use core::convert::TryFrom;

/// End-of-contents octets
///
/// `EndOfContent` is not a BER type, but represents a marked to indicate the end of contents
/// of an object, when the length is `Indefinite` (see X.690 section 8.1.5).
///
/// This type cannot exist in DER, and so provides no `FromDer`/`ToDer` implementation.
#[derive(Debug)]
pub struct EndOfContent {}

impl EndOfContent {
    pub const fn new() -> Self {
        EndOfContent {}
    }
}

impl<'a> TryFrom<Any<'a>> for EndOfContent {
    type Error = Error;

    fn try_from(any: Any<'a>) -> Result<EndOfContent> {
        TryFrom::try_from(&any)
    }
}

impl<'a, 'b> TryFrom<&'b Any<'a>> for EndOfContent {
    type Error = Error;

    fn try_from(any: &'b Any<'a>) -> Result<EndOfContent> {
        any.tag().assert_eq(Self::TAG)?;
        if !any.header.length.is_null() {
            return Err(Error::InvalidLength);
        }
        Ok(EndOfContent {})
    }
}

impl<'a> Tagged for EndOfContent {
    const TAG: Tag = Tag::EndOfContent;
}

// impl ToDer for EndOfContent {
//     fn to_der_len(&self) -> Result<usize> {
//         Ok(2)
//     }

//     fn write_der_header(&self, writer: &mut dyn std::io::Write) -> crate::SerializeResult<usize> {
//         writer.write(&[Self::TAG.0 as u8, 0x00]).map_err(Into::into)
//     }

//     fn write_der_content(&self, _writer: &mut dyn std::io::Write) -> crate::SerializeResult<usize> {
//         Ok(0)
//     }
// }

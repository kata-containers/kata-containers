use crate::{Error, Result};
use alloc::string::ToString;
use rusticata_macros::newtype_enum;

/// BER/DER Tag as defined in X.680 section 8.4
///
/// X.690 doesn't specify the maximum tag size so we're assuming that people
/// aren't going to need anything more than a u32.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tag(pub u32);

newtype_enum! {
impl display Tag {
    EndOfContent = 0,
    Boolean = 1,
    Integer = 2,
    BitString = 3,
    OctetString = 4,
    Null = 5,
    Oid = 6,
    ObjectDescriptor = 7,
    External = 8,
    RealType = 9,
    Enumerated = 10,
    EmbeddedPdv = 11,
    Utf8String = 12,
    RelativeOid = 13,

    Sequence = 16,
    Set = 17,
    NumericString = 18,
    PrintableString = 19,
    T61String = 20,
    TeletexString = 20,
    VideotexString = 21,

    Ia5String = 22,
    UtcTime = 23,
    GeneralizedTime = 24,

    GraphicString = 25,
    VisibleString = 26,
    GeneralString = 27,

    UniversalString = 28,
    BmpString = 30,
}
}

impl Tag {
    pub const fn assert_eq(&self, tag: Tag) -> Result<()> {
        if self.0 == tag.0 {
            Ok(())
        } else {
            Err(Error::UnexpectedTag {
                expected: Some(tag),
                actual: *self,
            })
        }
    }

    pub fn invalid_value(&self, msg: &str) -> Error {
        Error::InvalidValue {
            tag: *self,
            msg: msg.to_string(),
        }
    }
}

impl From<u32> for Tag {
    fn from(v: u32) -> Self {
        Tag(v)
    }
}

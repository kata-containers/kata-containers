use crate::misc::WriteExt;
use crate::{Result, Serializer};
use picky_asn1::tag::Tag;

/// A serializer for UTF-8 strings
pub struct Utf8String;
impl Utf8String {
    /// Serializes `value` into `writer`
    pub fn serialize(value: &str, ser: &mut Serializer) -> Result<usize> {
        let mut written = ser.h_write_header(Tag::UTF8_STRING, value.len())?;
        written += ser.writer.write_exact(value.as_bytes())?;
        Ok(written)
    }
}

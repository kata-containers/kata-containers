use crate::misc::WriteExt;
use crate::{Result, Serializer};
use picky_asn1::tag::Tag;

/// A serializer for booleans
pub struct Boolean;
impl Boolean {
    /// Serializes `value` into `writer`
    pub fn serialize(value: bool, ser: &mut Serializer) -> Result<usize> {
        let mut written = ser.h_write_header(Tag::BOOLEAN, 1)?;

        // Serialize the value
        written += if value {
            ser.writer.write_one(0xff)?
        } else {
            ser.writer.write_one(0x00)?
        };

        Ok(written)
    }
}

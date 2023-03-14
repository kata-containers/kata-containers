use crate::{Result, Serializer};
use picky_asn1::tag::Tag;

/// A serializer for the `Null` type
pub struct Null;
impl Null {
    /// Serializes a `Null` into `_writer`
    pub fn serialize(ser: &mut Serializer) -> Result<usize> {
        ser.h_write_header(Tag::NULL, 0)
    }
}

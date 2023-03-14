use crate::{Asn1DerError, Result};
use std::str;

/// A deserializer for UTF-8 strings
pub struct Utf8String;
impl Utf8String {
    /// The deserialized string for `data`
    pub fn deserialize(data: &[u8]) -> Result<&str> {
        str::from_utf8(data).map_err(|_| Asn1DerError::InvalidData)
    }
}

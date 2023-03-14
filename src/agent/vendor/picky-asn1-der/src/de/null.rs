use crate::{Asn1DerError, Result};

/// A deserializer for the `Null` type
pub struct Null;
impl Null {
    /// Deserializes `Null` from `data`
    pub fn deserialize(data: &[u8]) -> Result<()> {
        if !data.is_empty() {
            return Err(Asn1DerError::InvalidData);
        }
        Ok(())
    }
}

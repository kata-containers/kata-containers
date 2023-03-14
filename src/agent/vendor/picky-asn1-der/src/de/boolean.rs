use crate::{Asn1DerError, Result};

/// A deserializer for booleans
pub struct Boolean;
impl Boolean {
    /// The deserialized boolean for `data`
    pub fn deserialize(data: &[u8]) -> Result<bool> {
        // Check lengths
        if data.is_empty() {
            return Err(Asn1DerError::TruncatedData);
        }
        if data.len() > 1 {
            return Err(Asn1DerError::InvalidData);
        }

        // Parse the boolean
        Ok(match data[0] {
            0x00 => {
                debug_log!("false!");
                false
            }
            0xff => {
                debug_log!("true!");
                true
            }
            _ => return Err(Asn1DerError::InvalidData),
        })
    }
}

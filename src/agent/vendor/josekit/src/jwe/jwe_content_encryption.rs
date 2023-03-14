use std::cmp::Eq;
use std::fmt::Debug;

use crate::JoseError;

/// Represent a algorithm of JWE enc header claim.
pub trait JweContentEncryption: Debug + Send + Sync {
    /// Return the "enc" (encryption) header parameter value of JWE.
    fn name(&self) -> &str;

    fn key_len(&self) -> usize;

    fn iv_len(&self) -> usize;

    fn encrypt(
        &self,
        key: &[u8],
        iv: Option<&[u8]>,
        message: &[u8],
        aad: &[u8],
    ) -> Result<(Vec<u8>, Option<Vec<u8>>), JoseError>;

    fn decrypt(
        &self,
        key: &[u8],
        iv: Option<&[u8]>,
        encrypted_message: &[u8],
        aad: &[u8],
        tag: Option<&[u8]>,
    ) -> Result<Vec<u8>, JoseError>;

    fn box_clone(&self) -> Box<dyn JweContentEncryption>;
}

impl PartialEq for Box<dyn JweContentEncryption> {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl Eq for Box<dyn JweContentEncryption> {}

impl Clone for Box<dyn JweContentEncryption> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

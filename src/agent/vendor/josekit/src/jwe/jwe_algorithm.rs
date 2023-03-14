use std::borrow::Cow;
use std::fmt::Debug;

use crate::jwe::{JweContentEncryption, JweHeader};
use crate::JoseError;

/// Represent a algorithm of JWE alg header claim.
pub trait JweAlgorithm: Debug + Send + Sync {
    /// Return the "alg" (algorithm) header parameter value of JWE.
    fn name(&self) -> &str;

    fn box_clone(&self) -> Box<dyn JweAlgorithm>;
}

impl PartialEq for Box<dyn JweAlgorithm> {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl Eq for Box<dyn JweAlgorithm> {}

impl Clone for Box<dyn JweAlgorithm> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

pub trait JweEncrypter: Debug + Send + Sync {
    /// Return the source algorithm instance.
    fn algorithm(&self) -> &dyn JweAlgorithm;

    /// Return the source key ID.
    /// The default value is a value of kid parameter in JWK.
    fn key_id(&self) -> Option<&str>;

    /// Compute a content encryption key.
    ///
    /// # Arguments
    ///
    /// * `cencryption` - The content encryption method.
    /// * `in_header` - the input header
    /// * `out_header` - the output header
    fn compute_content_encryption_key(
        &self,
        cencryption: &dyn JweContentEncryption,
        in_header: &JweHeader,
        out_header: &mut JweHeader,
    ) -> Result<Option<Cow<[u8]>>, JoseError>;

    /// Return a encypted key.
    ///
    /// # Arguments
    ///
    /// * `key` - The content encryption key
    /// * `in_header` - the input header
    /// * `out_header` - the output header
    fn encrypt(
        &self,
        key: &[u8],
        in_header: &JweHeader,
        out_header: &mut JweHeader,
    ) -> Result<Option<Vec<u8>>, JoseError>;

    fn box_clone(&self) -> Box<dyn JweEncrypter>;
}

impl Clone for Box<dyn JweEncrypter> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

pub trait JweDecrypter: Debug + Send + Sync {
    /// Return the source algorithm instance.
    fn algorithm(&self) -> &dyn JweAlgorithm;

    /// Return the source key ID.
    /// The default value is a value of kid parameter in JWK.
    fn key_id(&self) -> Option<&str>;

    /// Return a decrypted key.
    ///
    /// # Arguments
    ///
    /// * `encrypted_key` - The encrypted key.
    /// * `cencryption` - The content encryption method.
    /// * `header` - The header
    fn decrypt(
        &self,
        encrypted_key: Option<&[u8]>,
        cencryption: &dyn JweContentEncryption,
        header: &JweHeader,
    ) -> Result<Cow<[u8]>, JoseError>;

    fn box_clone(&self) -> Box<dyn JweDecrypter>;
}

impl Clone for Box<dyn JweDecrypter> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

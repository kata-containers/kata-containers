use std::fmt::Debug;

use crate::JoseError;

pub trait JwsAlgorithm: Debug + Send + Sync {
    /// Return the "alg" (algorithm) header parameter value of JWS.
    fn name(&self) -> &str;

    fn box_clone(&self) -> Box<dyn JwsAlgorithm>;
}

impl PartialEq for Box<dyn JwsAlgorithm> {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl Eq for Box<dyn JwsAlgorithm> {}

impl Clone for Box<dyn JwsAlgorithm> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

pub trait JwsSigner: Debug + Send + Sync {
    /// Return the source algorithm instance.
    fn algorithm(&self) -> &dyn JwsAlgorithm;

    /// Return the source key ID.
    /// The default value is a value of kid parameter in JWK.
    fn key_id(&self) -> Option<&str>;

    /// Return the signature length of JWS.
    fn signature_len(&self) -> usize;

    /// Return a signature of the data.
    ///
    /// # Arguments
    ///
    /// * `message` - The message data to sign.
    fn sign(&self, message: &[u8]) -> Result<Vec<u8>, JoseError>;

    fn box_clone(&self) -> Box<dyn JwsSigner>;
}

impl Clone for Box<dyn JwsSigner> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

pub trait JwsVerifier: Debug + Send + Sync {
    /// Return the source algrithm instance.
    fn algorithm(&self) -> &dyn JwsAlgorithm;

    /// Return the source key ID.
    /// The default value is a value of kid parameter in JWK.
    fn key_id(&self) -> Option<&str>;

    /// Verify the data by the signature.
    ///
    /// # Arguments
    ///
    /// * `message` - a message data to verify.
    /// * `signature` - a signature data.
    fn verify(&self, message: &[u8], signature: &[u8]) -> Result<(), JoseError>;

    fn box_clone(&self) -> Box<dyn JwsVerifier>;
}

impl Clone for Box<dyn JwsVerifier> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

use std::fmt::Debug;

use crate::jwk::Jwk;

pub trait KeyPair: Debug + Send + Sync {
    /// Return the applicatable algorithm.
    fn algorithm(&self) -> Option<&str>;

    /// Return the applicatable key ID.
    fn key_id(&self) -> Option<&str>;

    fn to_der_private_key(&self) -> Vec<u8>;
    fn to_der_public_key(&self) -> Vec<u8>;
    fn to_pem_private_key(&self) -> Vec<u8>;
    fn to_pem_public_key(&self) -> Vec<u8>;
    fn to_jwk_private_key(&self) -> Jwk;
    fn to_jwk_public_key(&self) -> Jwk;
    fn to_jwk_key_pair(&self) -> Jwk;

    fn box_clone(&self) -> Box<dyn KeyPair>;
}

impl PartialEq for Box<dyn KeyPair> {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl Eq for Box<dyn KeyPair> {}

impl Clone for Box<dyn KeyPair> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

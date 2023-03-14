use std::ops::Deref;

use anyhow::bail;

use crate::jws::{JwsAlgorithm, JwsSigner, JwsVerifier};
use crate::JoseError;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub enum UnsecuredJwsAlgorithm {
    None,
}

impl UnsecuredJwsAlgorithm {
    pub fn signer(&self) -> UnsecuredJwsSigner {
        UnsecuredJwsSigner {
            algorithm: self.clone(),
        }
    }

    pub fn verifier(&self) -> UnsecuredJwsVerifier {
        UnsecuredJwsVerifier {
            algorithm: self.clone(),
        }
    }
}

impl JwsAlgorithm for UnsecuredJwsAlgorithm {
    fn name(&self) -> &str {
        "none"
    }

    fn box_clone(&self) -> Box<dyn JwsAlgorithm> {
        Box::new(self.clone())
    }
}

impl Deref for UnsecuredJwsAlgorithm {
    type Target = dyn JwsAlgorithm;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct UnsecuredJwsSigner {
    algorithm: UnsecuredJwsAlgorithm,
}

impl JwsSigner for UnsecuredJwsSigner {
    fn algorithm(&self) -> &dyn JwsAlgorithm {
        &self.algorithm
    }

    fn key_id(&self) -> Option<&str> {
        None
    }

    fn signature_len(&self) -> usize {
        0
    }

    fn sign(&self, _message: &[u8]) -> Result<Vec<u8>, JoseError> {
        Ok(vec![])
    }

    fn box_clone(&self) -> Box<dyn JwsSigner> {
        Box::new(self.clone())
    }
}

impl Deref for UnsecuredJwsSigner {
    type Target = dyn JwsSigner;

    fn deref(&self) -> &Self::Target {
        self
    }
}

#[derive(Debug, Clone)]
pub struct UnsecuredJwsVerifier {
    algorithm: UnsecuredJwsAlgorithm,
}

impl JwsVerifier for UnsecuredJwsVerifier {
    fn algorithm(&self) -> &dyn JwsAlgorithm {
        &self.algorithm
    }

    fn key_id(&self) -> Option<&str> {
        None
    }

    fn verify(&self, _message: &[u8], signature: &[u8]) -> Result<(), JoseError> {
        (|| -> anyhow::Result<()> {
            if signature.len() != 0 {
                bail!(
                    "The length of none algorithm signature must be 0: {}",
                    signature.len()
                );
            }

            Ok(())
        })()
        .map_err(|err| JoseError::InvalidSignature(err))
    }

    fn box_clone(&self) -> Box<dyn JwsVerifier> {
        Box::new(self.clone())
    }
}

impl Deref for UnsecuredJwsVerifier {
    type Target = dyn JwsVerifier;

    fn deref(&self) -> &Self::Target {
        self
    }
}

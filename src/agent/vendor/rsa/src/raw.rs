use alloc::vec::Vec;
use num_bigint::BigUint;
use rand_core::{CryptoRng, RngCore};
use zeroize::Zeroize;

use crate::errors::{Error, Result};
use crate::internals;
use crate::key::{RsaPrivateKey, RsaPublicKey};

pub trait EncryptionPrimitive {
    /// Do NOT use directly! Only for implementors.
    fn raw_encryption_primitive(&self, plaintext: &[u8], pad_size: usize) -> Result<Vec<u8>>;
}

pub trait DecryptionPrimitive {
    /// Do NOT use directly! Only for implementors.
    fn raw_decryption_primitive<R: RngCore + CryptoRng>(
        &self,
        rng: Option<&mut R>,
        ciphertext: &[u8],
        pad_size: usize,
    ) -> Result<Vec<u8>>;
}

impl EncryptionPrimitive for RsaPublicKey {
    fn raw_encryption_primitive(&self, plaintext: &[u8], pad_size: usize) -> Result<Vec<u8>> {
        let mut m = BigUint::from_bytes_be(plaintext);
        let mut c = internals::encrypt(self, &m);
        let mut c_bytes = c.to_bytes_be();
        let ciphertext = internals::left_pad(&c_bytes, pad_size);

        if pad_size < ciphertext.len() {
            return Err(Error::Verification);
        }

        // clear out tmp values
        m.zeroize();
        c.zeroize();
        c_bytes.zeroize();

        Ok(ciphertext)
    }
}

impl<'a> EncryptionPrimitive for &'a RsaPublicKey {
    fn raw_encryption_primitive(&self, plaintext: &[u8], pad_size: usize) -> Result<Vec<u8>> {
        (*self).raw_encryption_primitive(plaintext, pad_size)
    }
}

impl DecryptionPrimitive for RsaPrivateKey {
    fn raw_decryption_primitive<R: RngCore + CryptoRng>(
        &self,
        rng: Option<&mut R>,
        ciphertext: &[u8],
        pad_size: usize,
    ) -> Result<Vec<u8>> {
        let mut c = BigUint::from_bytes_be(ciphertext);
        let mut m = internals::decrypt_and_check(rng, self, &c)?;
        let mut m_bytes = m.to_bytes_be();
        let plaintext = internals::left_pad(&m_bytes, pad_size);

        // clear tmp values
        c.zeroize();
        m.zeroize();
        m_bytes.zeroize();

        Ok(plaintext)
    }
}

impl<'a> DecryptionPrimitive for &'a RsaPrivateKey {
    fn raw_decryption_primitive<R: RngCore + CryptoRng>(
        &self,
        rng: Option<&mut R>,
        ciphertext: &[u8],
        pad_size: usize,
    ) -> Result<Vec<u8>> {
        (*self).raw_decryption_primitive(rng, ciphertext, pad_size)
    }
}

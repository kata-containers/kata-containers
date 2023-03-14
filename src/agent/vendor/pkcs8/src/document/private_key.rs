//! PKCS#8 private key document.

use crate::{DecodePrivateKey, EncodePrivateKey, Error, PrivateKeyInfo, Result};
use alloc::vec::Vec;
use core::fmt;
use der::{Decodable, Document};
use zeroize::{Zeroize, Zeroizing};

#[cfg(feature = "encryption")]
use {
    crate::{EncryptedPrivateKeyDocument, EncryptedPrivateKeyInfo},
    pkcs5::pbes2,
    rand_core::{CryptoRng, RngCore},
};

#[cfg(feature = "pem")]
use {
    alloc::string::String,
    core::str::FromStr,
    der::pem::{self, LineEnding},
};

#[cfg(feature = "std")]
use std::path::Path;

/// PKCS#8 private key document.
///
/// This type provides storage for [`PrivateKeyInfo`] encoded as ASN.1 DER
/// with the invariant that the contained-document is "well-formed", i.e. it
/// will parse successfully according to this crate's parsing rules.
#[derive(Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct PrivateKeyDocument(Zeroizing<Vec<u8>>);

impl<'a> Document<'a> for PrivateKeyDocument {
    type Message = PrivateKeyInfo<'a>;
    const SENSITIVE: bool = true;
}

impl PrivateKeyDocument {
    /// Encrypt this private key using a symmetric encryption key derived
    /// from the provided password.
    ///
    /// Uses the following algorithms for encryption:
    /// - PBKDF: scrypt with default parameters:
    ///   - log₂(N): 15
    ///   - r: 8
    ///   - p: 1
    /// - Cipher: AES-256-CBC (best available option for PKCS#5 encryption)
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    pub fn encrypt(
        &self,
        mut rng: impl CryptoRng + RngCore,
        password: impl AsRef<[u8]>,
    ) -> Result<EncryptedPrivateKeyDocument> {
        let mut salt = [0u8; 16];
        rng.fill_bytes(&mut salt);

        let mut iv = [0u8; 16];
        rng.fill_bytes(&mut iv);

        let pbes2_params = pbes2::Parameters::scrypt_aes256cbc(Default::default(), &salt, &iv)?;
        self.encrypt_with_params(pbes2_params, password)
    }

    /// Encrypt this private key using a symmetric encryption key derived
    /// from the provided password and [`pbes2::Parameters`].
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    pub fn encrypt_with_params(
        &self,
        pbes2_params: pbes2::Parameters<'_>,
        password: impl AsRef<[u8]>,
    ) -> Result<EncryptedPrivateKeyDocument> {
        let encrypted_data = pbes2_params.encrypt(password, self.as_ref())?;

        EncryptedPrivateKeyInfo {
            encryption_algorithm: pbes2_params.into(),
            encrypted_data: &encrypted_data,
        }
        .try_into()
    }
}

impl DecodePrivateKey for PrivateKeyDocument {
    fn from_pkcs8_der(bytes: &[u8]) -> Result<Self> {
        Ok(Self::from_der(bytes)?)
    }

    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn from_pkcs8_pem(s: &str) -> Result<Self> {
        Ok(Self::from_pem(s)?)
    }

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_pkcs8_der_file(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::read_der_file(path)?)
    }

    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_pkcs8_pem_file(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::read_pem_file(path)?)
    }
}

impl EncodePrivateKey for PrivateKeyDocument {
    fn to_pkcs8_der(&self) -> Result<PrivateKeyDocument> {
        Ok(self.clone())
    }

    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn to_pkcs8_pem(&self, line_ending: LineEnding) -> Result<Zeroizing<String>> {
        Ok(Zeroizing::new(self.to_pem(line_ending)?))
    }

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_pkcs8_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(self.write_der_file(path)?)
    }

    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_pkcs8_pem_file(&self, path: impl AsRef<Path>, line_ending: LineEnding) -> Result<()> {
        Ok(self.write_pem_file(path, line_ending)?)
    }
}

impl AsRef<[u8]> for PrivateKeyDocument {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl TryFrom<PrivateKeyInfo<'_>> for PrivateKeyDocument {
    type Error = Error;

    fn try_from(private_key_info: PrivateKeyInfo<'_>) -> Result<PrivateKeyDocument> {
        Self::try_from(&private_key_info)
    }
}

impl TryFrom<&PrivateKeyInfo<'_>> for PrivateKeyDocument {
    type Error = Error;

    fn try_from(private_key_info: &PrivateKeyInfo<'_>) -> Result<PrivateKeyDocument> {
        Ok(Self::from_msg(private_key_info)?)
    }
}

impl TryFrom<&[u8]> for PrivateKeyDocument {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        PrivateKeyDocument::from_pkcs8_der(bytes)
    }
}

impl TryFrom<Vec<u8>> for PrivateKeyDocument {
    type Error = der::Error;

    fn try_from(mut bytes: Vec<u8>) -> der::Result<Self> {
        // Ensure document is well-formed
        if let Err(err) = PrivateKeyInfo::from_der(bytes.as_slice()) {
            bytes.zeroize();
            return Err(err);
        }

        Ok(Self(Zeroizing::new(bytes)))
    }
}

impl fmt::Debug for PrivateKeyDocument {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("PrivateKeyDocument")
            .field(&self.decode())
            .finish()
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl FromStr for PrivateKeyDocument {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_pkcs8_pem(s)
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl pem::PemLabel for PrivateKeyDocument {
    const TYPE_LABEL: &'static str = "PRIVATE KEY";
}

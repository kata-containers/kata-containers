//! Traits for parsing objects from PKCS#8 encoded documents

use crate::{Error, PrivateKeyInfo, Result};

#[cfg(feature = "alloc")]
use {crate::PrivateKeyDocument, der::Document};

#[cfg(feature = "encryption")]
use {
    crate::{EncryptedPrivateKeyDocument, EncryptedPrivateKeyInfo},
    rand_core::{CryptoRng, RngCore},
};

#[cfg(feature = "std")]
use std::path::Path;
#[cfg(feature = "pem")]
use {crate::LineEnding, alloc::string::String, zeroize::Zeroizing};

/// Parse a private key object from a PKCS#8 encoded document.
pub trait DecodePrivateKey: for<'a> TryFrom<PrivateKeyInfo<'a>, Error = Error> + Sized {
    /// Deserialize PKCS#8 private key from ASN.1 DER-encoded data
    /// (binary format).
    fn from_pkcs8_der(bytes: &[u8]) -> Result<Self> {
        Self::try_from(PrivateKeyInfo::try_from(bytes)?)
    }

    /// Deserialize encrypted PKCS#8 private key from ASN.1 DER-encoded data
    /// (binary format) and attempt to decrypt it using the provided password.
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    fn from_pkcs8_encrypted_der(bytes: &[u8], password: impl AsRef<[u8]>) -> Result<Self> {
        EncryptedPrivateKeyInfo::try_from(bytes)?
            .decrypt(password)
            .and_then(|doc| Self::from_pkcs8_doc(&doc))
    }

    /// Deserialize PKCS#8 private key from a [`PrivateKeyDocument`].
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    fn from_pkcs8_doc(doc: &PrivateKeyDocument) -> Result<Self> {
        Self::try_from(doc.decode())
    }

    /// Deserialize PKCS#8-encoded private key from PEM.
    ///
    /// Keys in this format begin with the following delimiter:
    ///
    /// ```text
    /// -----BEGIN PRIVATE KEY-----
    /// ```
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn from_pkcs8_pem(s: &str) -> Result<Self> {
        PrivateKeyDocument::from_pkcs8_pem(s).and_then(|doc| Self::from_pkcs8_doc(&doc))
    }

    /// Deserialize encrypted PKCS#8-encoded private key from PEM and attempt
    /// to decrypt it using the provided password.
    ///
    /// Keys in this format begin with the following delimiter:
    ///
    /// ```text
    /// -----BEGIN ENCRYPTED PRIVATE KEY-----
    /// ```
    #[cfg(all(feature = "encryption", feature = "pem"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "encryption", feature = "pem"))))]
    fn from_pkcs8_encrypted_pem(s: &str, password: impl AsRef<[u8]>) -> Result<Self> {
        EncryptedPrivateKeyDocument::from_pem(s)?
            .decrypt(password)
            .and_then(|doc| Self::from_pkcs8_doc(&doc))
    }

    /// Load PKCS#8 private key from an ASN.1 DER-encoded file on the local
    /// filesystem (binary format).
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_pkcs8_der_file(path: impl AsRef<Path>) -> Result<Self> {
        PrivateKeyDocument::read_pkcs8_der_file(path).and_then(|doc| Self::from_pkcs8_doc(&doc))
    }

    /// Load PKCS#8 private key from a PEM-encoded file on the local filesystem.
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_pkcs8_pem_file(path: impl AsRef<Path>) -> Result<Self> {
        PrivateKeyDocument::read_pkcs8_pem_file(path).and_then(|doc| Self::from_pkcs8_doc(&doc))
    }
}

/// Serialize a private key object to a PKCS#8 encoded document.
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub trait EncodePrivateKey {
    /// Serialize a [`PrivateKeyDocument`] containing a PKCS#8-encoded private key.
    fn to_pkcs8_der(&self) -> Result<PrivateKeyDocument>;

    /// Create an [`EncryptedPrivateKeyDocument`] containing the ciphertext of
    /// a PKCS#8 encoded private key encrypted under the given `password`.
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    fn to_pkcs8_encrypted_der(
        &self,
        rng: impl CryptoRng + RngCore,
        password: impl AsRef<[u8]>,
    ) -> Result<EncryptedPrivateKeyDocument> {
        self.to_pkcs8_der()?.encrypt(rng, password)
    }

    /// Serialize this private key as PEM-encoded PKCS#8 with the given [`LineEnding`].
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn to_pkcs8_pem(&self, line_ending: LineEnding) -> Result<Zeroizing<String>> {
        self.to_pkcs8_der()?.to_pkcs8_pem(line_ending)
    }

    /// Serialize this private key as an encrypted PEM-encoded PKCS#8 private
    /// key using the `provided` to derive an encryption key.
    #[cfg(all(feature = "encryption", feature = "pem"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "encryption", feature = "pem"))))]
    fn to_pkcs8_encrypted_pem(
        &self,
        rng: impl CryptoRng + RngCore,
        password: impl AsRef<[u8]>,
        line_ending: LineEnding,
    ) -> Result<Zeroizing<String>> {
        Ok(Zeroizing::new(
            self.to_pkcs8_encrypted_der(rng, password)?
                .to_pem(line_ending)?,
        ))
    }

    /// Write ASN.1 DER-encoded PKCS#8 private key to the given path
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_pkcs8_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        self.to_pkcs8_der()?.write_pkcs8_der_file(path)
    }

    /// Write ASN.1 DER-encoded PKCS#8 private key to the given path
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "pem", feature = "std"))))]
    fn write_pkcs8_pem_file(&self, path: impl AsRef<Path>, line_ending: LineEnding) -> Result<()> {
        self.to_pkcs8_der()?.write_pkcs8_pem_file(path, line_ending)
    }
}

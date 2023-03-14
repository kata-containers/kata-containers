//! Traits for encoding/decoding SPKI public keys.

use crate::{Error, Result, SubjectPublicKeyInfo};

#[cfg(feature = "alloc")]
use {crate::PublicKeyDocument, der::Document};

#[cfg(feature = "pem")]
use {alloc::string::String, der::pem::LineEnding};

#[cfg(feature = "std")]
use std::path::Path;

/// Parse a public key object from an encoded SPKI document.
pub trait DecodePublicKey:
    for<'a> TryFrom<SubjectPublicKeyInfo<'a>, Error = Error> + Sized
{
    /// Deserialize object from ASN.1 DER-encoded [`SubjectPublicKeyInfo`]
    /// (binary format).
    fn from_public_key_der(bytes: &[u8]) -> Result<Self> {
        Self::try_from(SubjectPublicKeyInfo::try_from(bytes)?)
    }

    /// Deserialize SPKI public key from a [`PublicKeyDocument`].
    #[cfg(feature = "alloc")]
    #[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
    fn from_public_key_doc(doc: &PublicKeyDocument) -> Result<Self> {
        Self::try_from(doc.decode())
    }

    /// Deserialize PEM-encoded [`SubjectPublicKeyInfo`].
    ///
    /// Keys in this format begin with the following delimiter:
    ///
    /// ```text
    /// -----BEGIN PUBLIC KEY-----
    /// ```
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn from_public_key_pem(s: &str) -> Result<Self> {
        PublicKeyDocument::from_public_key_pem(s).and_then(|doc| Self::from_public_key_doc(&doc))
    }

    /// Load public key object from an ASN.1 DER-encoded file on the local
    /// filesystem (binary format).
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_public_key_der_file(path: impl AsRef<Path>) -> Result<Self> {
        PublicKeyDocument::read_public_key_der_file(path)
            .and_then(|doc| Self::from_public_key_doc(&doc))
    }

    /// Load public key object from a PEM-encoded file on the local filesystem.
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "pem", feature = "std"))))]
    fn read_public_key_pem_file(path: impl AsRef<Path>) -> Result<Self> {
        PublicKeyDocument::read_public_key_pem_file(path)
            .and_then(|doc| Self::from_public_key_doc(&doc))
    }
}

/// Serialize a public key object to a SPKI-encoded document.
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub trait EncodePublicKey {
    /// Serialize a [`PublicKeyDocument`] containing a SPKI-encoded public key.
    fn to_public_key_der(&self) -> Result<PublicKeyDocument>;

    /// Serialize this public key as PEM-encoded SPKI with the given [`LineEnding`].
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn to_public_key_pem(&self, line_ending: LineEnding) -> Result<String> {
        self.to_public_key_der()?.to_public_key_pem(line_ending)
    }

    /// Write ASN.1 DER-encoded public key to the given path
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_public_key_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        self.to_public_key_der()?.write_public_key_der_file(path)
    }

    /// Write ASN.1 DER-encoded public key to the given path
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "pem", feature = "std"))))]
    fn write_public_key_pem_file(
        &self,
        path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<()> {
        self.to_public_key_der()?
            .write_public_key_pem_file(path, line_ending)
    }
}

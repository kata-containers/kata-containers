//! Traits for parsing objects from PKCS#1 encoded documents

use crate::Result;

#[cfg(feature = "alloc")]
use crate::{RsaPrivateKeyDocument, RsaPublicKeyDocument};

#[cfg(feature = "pem")]
use {crate::LineEnding, alloc::string::String};

#[cfg(feature = "pkcs8")]
use crate::{ALGORITHM_ID, ALGORITHM_OID};

#[cfg(feature = "std")]
use std::path::Path;

#[cfg(all(feature = "alloc", feature = "pkcs8"))]
use der::Document;

#[cfg(any(feature = "pem", feature = "std"))]
use zeroize::Zeroizing;

/// Parse an [`RsaPrivateKey`] from a PKCS#1-encoded document.
pub trait DecodeRsaPrivateKey: Sized {
    /// Deserialize PKCS#1 private key from ASN.1 DER-encoded data
    /// (binary format).
    fn from_pkcs1_der(bytes: &[u8]) -> Result<Self>;

    /// Deserialize PKCS#1-encoded private key from PEM.
    ///
    /// Keys in this format begin with the following:
    ///
    /// ```text
    /// -----BEGIN RSA PRIVATE KEY-----
    /// ```
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn from_pkcs1_pem(s: &str) -> Result<Self> {
        RsaPrivateKeyDocument::from_pkcs1_pem(s).and_then(|doc| Self::from_pkcs1_der(doc.as_der()))
    }

    /// Load PKCS#1 private key from an ASN.1 DER-encoded file on the local
    /// filesystem (binary format).
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_pkcs1_der_file(path: impl AsRef<Path>) -> Result<Self> {
        RsaPrivateKeyDocument::read_pkcs1_der_file(path)
            .and_then(|doc| Self::from_pkcs1_der(doc.as_der()))
    }

    /// Load PKCS#1 private key from a PEM-encoded file on the local filesystem.
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_pkcs1_pem_file(path: impl AsRef<Path>) -> Result<Self> {
        RsaPrivateKeyDocument::read_pkcs1_pem_file(path)
            .and_then(|doc| Self::from_pkcs1_der(doc.as_der()))
    }
}

/// Parse a [`RsaPublicKey`] from a PKCS#1-encoded document.
pub trait DecodeRsaPublicKey: Sized {
    /// Deserialize object from ASN.1 DER-encoded [`RsaPublicKey`]
    /// (binary format).
    fn from_pkcs1_der(bytes: &[u8]) -> Result<Self>;

    /// Deserialize PEM-encoded [`RsaPublicKey`].
    ///
    /// Keys in this format begin with the following:
    ///
    /// ```text
    /// -----BEGIN RSA PUBLIC KEY-----
    /// ```
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn from_pkcs1_pem(s: &str) -> Result<Self> {
        RsaPublicKeyDocument::from_pkcs1_pem(s).and_then(|doc| Self::from_pkcs1_der(doc.as_der()))
    }

    /// Load [`RsaPublicKey`] from an ASN.1 DER-encoded file on the local
    /// filesystem (binary format).
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_pkcs1_der_file(path: impl AsRef<Path>) -> Result<Self> {
        RsaPublicKeyDocument::read_pkcs1_der_file(path)
            .and_then(|doc| Self::from_pkcs1_der(doc.as_der()))
    }

    /// Load [`RsaPublicKey`] from a PEM-encoded file on the local filesystem.
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_pkcs1_pem_file(path: impl AsRef<Path>) -> Result<Self> {
        RsaPublicKeyDocument::read_pkcs1_pem_file(path)
            .and_then(|doc| Self::from_pkcs1_der(doc.as_der()))
    }
}

/// Serialize a [`RsaPrivateKey`] to a PKCS#1 encoded document.
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub trait EncodeRsaPrivateKey {
    /// Serialize a [`RsaPrivateKeyDocument`] containing a PKCS#1-encoded private key.
    fn to_pkcs1_der(&self) -> Result<RsaPrivateKeyDocument>;

    /// Serialize this private key as PEM-encoded PKCS#1 with the given [`LineEnding`].
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn to_pkcs1_pem(&self, line_ending: LineEnding) -> Result<Zeroizing<String>> {
        self.to_pkcs1_der()?.to_pkcs1_pem(line_ending)
    }

    /// Write ASN.1 DER-encoded PKCS#1 private key to the given path.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_pkcs1_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        self.to_pkcs1_der()?.write_pkcs1_der_file(path)
    }

    /// Write ASN.1 DER-encoded PKCS#1 private key to the given path.
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "pem", feature = "std"))))]
    fn write_pkcs1_pem_file(&self, path: impl AsRef<Path>, line_ending: LineEnding) -> Result<()> {
        self.to_pkcs1_der()?.write_pkcs1_pem_file(path, line_ending)
    }
}

/// Serialize a [`RsaPublicKey`] to a PKCS#1-encoded document.
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub trait EncodeRsaPublicKey {
    /// Serialize a [`RsaPublicKeyDocument`] containing a PKCS#1-encoded public key.
    fn to_pkcs1_der(&self) -> Result<RsaPublicKeyDocument>;

    /// Serialize this public key as PEM-encoded PKCS#1 with the given line ending.
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn to_pkcs1_pem(&self, line_ending: LineEnding) -> Result<String> {
        self.to_pkcs1_der()?.to_pkcs1_pem(line_ending)
    }

    /// Write ASN.1 DER-encoded public key to the given path.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_pkcs1_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        self.to_pkcs1_der()?.write_pkcs1_der_file(path)
    }

    /// Write ASN.1 DER-encoded public key to the given path.
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "pem", feature = "std"))))]
    fn write_pkcs1_pem_file(&self, path: impl AsRef<Path>, line_ending: LineEnding) -> Result<()> {
        self.to_pkcs1_der()?.write_pkcs1_pem_file(path, line_ending)
    }
}

#[cfg(feature = "pkcs8")]
#[cfg_attr(docsrs, doc(cfg(feature = "pkcs8")))]
impl<T: pkcs8::DecodePrivateKey> DecodeRsaPrivateKey for T {
    fn from_pkcs1_der(private_key: &[u8]) -> Result<Self> {
        let algorithm = pkcs8::AlgorithmIdentifier {
            oid: ALGORITHM_OID,
            parameters: Some(der::asn1::Null.into()),
        };

        Ok(Self::try_from(pkcs8::PrivateKeyInfo {
            algorithm,
            private_key,
            public_key: None,
        })?)
    }
}

#[cfg(feature = "pkcs8")]
#[cfg_attr(docsrs, doc(cfg(feature = "pkcs8")))]
impl<T: pkcs8::DecodePublicKey> DecodeRsaPublicKey for T {
    fn from_pkcs1_der(public_key: &[u8]) -> Result<Self> {
        Ok(Self::try_from(pkcs8::SubjectPublicKeyInfo {
            algorithm: ALGORITHM_ID,
            subject_public_key: public_key,
        })?)
    }
}

#[cfg(all(feature = "alloc", feature = "pkcs8"))]
#[cfg_attr(docsrs, doc(all(feature = "alloc", feature = "pkcs8")))]
impl<T: pkcs8::EncodePrivateKey> EncodeRsaPrivateKey for T {
    fn to_pkcs1_der(&self) -> Result<RsaPrivateKeyDocument> {
        let doc = self.to_pkcs8_der()?;
        Ok(RsaPrivateKeyDocument::from_der(doc.decode().private_key)?)
    }
}

#[cfg(all(feature = "alloc", feature = "pkcs8"))]
#[cfg_attr(docsrs, doc(all(feature = "alloc", feature = "pkcs8")))]
impl<T: pkcs8::EncodePublicKey> EncodeRsaPublicKey for T {
    fn to_pkcs1_der(&self) -> Result<RsaPublicKeyDocument> {
        let doc = self.to_public_key_der()?;
        Ok(RsaPublicKeyDocument::from_der(
            doc.decode().subject_public_key,
        )?)
    }
}

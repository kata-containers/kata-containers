//! SPKI public key document.

use crate::{error, Error, Result, SubjectPublicKeyInfo};
use alloc::{borrow::ToOwned, vec::Vec};
use core::{
    convert::{TryFrom, TryInto},
    fmt,
};
use der::Encodable;

#[cfg(feature = "std")]
use std::{fs, path::Path, str};

#[cfg(feature = "pem")]
use {crate::pem, alloc::string::String, core::str::FromStr};

/// SPKI public key document.
///
/// This type provides storage for [`SubjectPublicKeyInfo`] encoded as ASN.1
/// DER with the invariant that the contained-document is "well-formed", i.e.
/// it will parse successfully according to this crate's parsing rules.
#[derive(Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct PublicKeyDocument(Vec<u8>);

impl PublicKeyDocument {
    /// Parse the [`SubjectPublicKeyInfo`] contained in this [`PublicKeyDocument`]
    pub fn spki(&self) -> SubjectPublicKeyInfo<'_> {
        SubjectPublicKeyInfo::try_from(self.0.as_slice()).expect("malformed PublicKeyDocument")
    }

    /// Parse [`PublicKeyDocument`] from ASN.1 DER
    pub fn from_der(bytes: &[u8]) -> Result<Self> {
        bytes.try_into()
    }

    /// Parse [`PublicKeyDocument`] from PEM
    ///
    /// PEM-encoded public keys can be identified by the leading delimiter:
    ///
    /// ```text
    /// -----BEGIN PUBLIC KEY-----
    /// ```
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    pub fn from_pem(s: &str) -> Result<Self> {
        let der_bytes = pem::decode(s, pem::PUBLIC_KEY_BOUNDARY)?;
        Self::from_der(&*der_bytes)
    }

    /// Serialize [`PublicKeyDocument`] as PEM-encoded PKCS#8 string.
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    pub fn to_pem(&self) -> String {
        pem::encode(&self.0, pem::PUBLIC_KEY_BOUNDARY)
    }

    /// Load [`PublicKeyDocument`] from an ASN.1 DER-encoded file on the local
    /// filesystem (binary format).
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn read_der_file(path: impl AsRef<Path>) -> Result<Self> {
        fs::read(path)?.try_into()
    }

    /// Load [`PublicKeyDocument`] from a PEM-encoded file on the local filesystem.
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn read_pem_file(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_pem(&fs::read_to_string(path)?)
    }

    /// Write ASN.1 DER-encoded public key to the given path
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn write_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        fs::write(path, self.as_ref())?;
        Ok(())
    }

    /// Write PEM-encoded public key to the given path
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn write_pem_file(&self, path: impl AsRef<Path>) -> Result<()> {
        fs::write(path, self.to_pem().as_bytes())?;
        Ok(())
    }
}

impl AsRef<[u8]> for PublicKeyDocument {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<SubjectPublicKeyInfo<'_>> for PublicKeyDocument {
    fn from(spki: SubjectPublicKeyInfo<'_>) -> PublicKeyDocument {
        PublicKeyDocument::from(&spki)
    }
}

impl From<&SubjectPublicKeyInfo<'_>> for PublicKeyDocument {
    fn from(spki: &SubjectPublicKeyInfo<'_>) -> PublicKeyDocument {
        spki.to_vec()
            .ok()
            .and_then(|buf| buf.try_into().ok())
            .expect(error::DER_ENCODING_MSG)
    }
}

impl TryFrom<&[u8]> for PublicKeyDocument {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        // Ensure document is well-formed
        SubjectPublicKeyInfo::try_from(bytes)?;
        Ok(Self(bytes.to_owned()))
    }
}

impl TryFrom<Vec<u8>> for PublicKeyDocument {
    type Error = Error;

    fn try_from(bytes: Vec<u8>) -> Result<Self> {
        // Ensure document is well-formed
        SubjectPublicKeyInfo::try_from(bytes.as_slice())?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for PublicKeyDocument {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("PublicKeyDocument")
            .field(&self.spki())
            .finish()
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl FromStr for PublicKeyDocument {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_pem(s)
    }
}

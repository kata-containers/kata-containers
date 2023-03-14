//! PKCS#1 RSA public key document.

use crate::{DecodeRsaPublicKey, EncodeRsaPublicKey, Error, Result, RsaPublicKey};
use alloc::vec::Vec;
use core::fmt;
use der::{Decodable, Document, Encodable};

#[cfg(feature = "pem")]
use {
    crate::{pem, LineEnding},
    alloc::string::String,
    core::str::FromStr,
};

#[cfg(feature = "std")]
use std::path::Path;

/// PKCS#1 `RSA PUBLIC KEY` document.
///
/// This type provides storage for [`RsaPublicKey`] encoded as ASN.1
/// DER with the invariant that the contained-document is "well-formed", i.e.
/// it will parse successfully according to this crate's parsing rules.
#[derive(Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct RsaPublicKeyDocument(Vec<u8>);

impl<'a> Document<'a> for RsaPublicKeyDocument {
    type Message = RsaPublicKey<'a>;
    const SENSITIVE: bool = false;
}

impl DecodeRsaPublicKey for RsaPublicKeyDocument {
    fn from_pkcs1_der(bytes: &[u8]) -> Result<Self> {
        Ok(Self::from_der(bytes)?)
    }

    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn from_pkcs1_pem(s: &str) -> Result<Self> {
        Ok(Self::from_pem(s)?)
    }

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_pkcs1_der_file(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::read_der_file(path)?)
    }

    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_pkcs1_pem_file(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::read_pem_file(path)?)
    }
}

impl EncodeRsaPublicKey for RsaPublicKeyDocument {
    fn to_pkcs1_der(&self) -> Result<RsaPublicKeyDocument> {
        Ok(self.clone())
    }

    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn to_pkcs1_pem(&self, line_ending: LineEnding) -> Result<String> {
        Ok(self.to_pem(line_ending)?)
    }

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_pkcs1_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(self.write_der_file(path)?)
    }

    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "pem", feature = "std"))))]
    fn write_pkcs1_pem_file(&self, path: impl AsRef<Path>, line_ending: LineEnding) -> Result<()> {
        Ok(self.write_pem_file(path, line_ending)?)
    }
}

impl AsRef<[u8]> for RsaPublicKeyDocument {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl TryFrom<&[u8]> for RsaPublicKeyDocument {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        Ok(Self::from_der(bytes)?)
    }
}

impl TryFrom<RsaPublicKey<'_>> for RsaPublicKeyDocument {
    type Error = Error;

    fn try_from(public_key: RsaPublicKey<'_>) -> Result<RsaPublicKeyDocument> {
        RsaPublicKeyDocument::try_from(&public_key)
    }
}

impl TryFrom<&RsaPublicKey<'_>> for RsaPublicKeyDocument {
    type Error = Error;

    fn try_from(public_key: &RsaPublicKey<'_>) -> Result<RsaPublicKeyDocument> {
        Ok(public_key.to_vec()?.try_into()?)
    }
}

impl TryFrom<Vec<u8>> for RsaPublicKeyDocument {
    type Error = der::Error;

    fn try_from(bytes: Vec<u8>) -> der::Result<Self> {
        // Ensure document is well-formed
        RsaPublicKey::from_der(bytes.as_slice())?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for RsaPublicKeyDocument {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("RsaPublicKeyDocument")
            .field(&self.decode())
            .finish()
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl FromStr for RsaPublicKeyDocument {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_pkcs1_pem(s)
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl pem::PemLabel for RsaPublicKeyDocument {
    const TYPE_LABEL: &'static str = "RSA PUBLIC KEY";
}

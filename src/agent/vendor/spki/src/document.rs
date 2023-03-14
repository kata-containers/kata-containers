//! SPKI public key document.

use crate::{DecodePublicKey, EncodePublicKey, Error, Result, SubjectPublicKeyInfo};
use alloc::vec::Vec;
use core::fmt;
use der::{Decodable, Document};

#[cfg(feature = "std")]
use std::path::Path;

#[cfg(feature = "pem")]
use {
    alloc::string::String,
    core::str::FromStr,
    der::pem::{self, LineEnding},
};

/// SPKI public key document.
///
/// This type provides storage for [`SubjectPublicKeyInfo`] encoded as ASN.1
/// DER with the invariant that the contained-document is "well-formed", i.e.
/// it will parse successfully according to this crate's parsing rules.
#[derive(Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct PublicKeyDocument(Vec<u8>);

impl<'a> Document<'a> for PublicKeyDocument {
    type Message = SubjectPublicKeyInfo<'a>;
    const SENSITIVE: bool = false;
}

impl DecodePublicKey for PublicKeyDocument {
    fn from_public_key_der(bytes: &[u8]) -> Result<Self> {
        Ok(Self::from_der(bytes)?)
    }

    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn from_public_key_pem(s: &str) -> Result<Self> {
        Ok(Self::from_pem(s)?)
    }

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_public_key_der_file(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::read_der_file(path)?)
    }

    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "pem", feature = "std"))))]
    fn read_public_key_pem_file(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::read_pem_file(path)?)
    }
}

impl EncodePublicKey for PublicKeyDocument {
    fn to_public_key_der(&self) -> Result<PublicKeyDocument> {
        Ok(self.clone())
    }

    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn to_public_key_pem(&self, line_ending: LineEnding) -> Result<String> {
        Ok(self.to_pem(line_ending)?)
    }

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_public_key_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(self.write_der_file(path)?)
    }

    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "pem", feature = "std"))))]
    fn write_public_key_pem_file(
        &self,
        path: impl AsRef<Path>,
        line_ending: LineEnding,
    ) -> Result<()> {
        Ok(self.write_pem_file(path, line_ending)?)
    }
}

impl AsRef<[u8]> for PublicKeyDocument {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl TryFrom<&[u8]> for PublicKeyDocument {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        Ok(Self::from_der(bytes)?)
    }
}

impl TryFrom<SubjectPublicKeyInfo<'_>> for PublicKeyDocument {
    type Error = Error;

    fn try_from(spki: SubjectPublicKeyInfo<'_>) -> Result<PublicKeyDocument> {
        Self::try_from(&spki)
    }
}

impl TryFrom<&SubjectPublicKeyInfo<'_>> for PublicKeyDocument {
    type Error = Error;

    fn try_from(spki: &SubjectPublicKeyInfo<'_>) -> Result<PublicKeyDocument> {
        Ok(Self::from_msg(spki)?)
    }
}

impl TryFrom<Vec<u8>> for PublicKeyDocument {
    type Error = der::Error;

    fn try_from(bytes: Vec<u8>) -> der::Result<Self> {
        // Ensure document is well-formed
        SubjectPublicKeyInfo::from_der(bytes.as_slice())?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for PublicKeyDocument {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("PublicKeyDocument")
            .field(&self.decode())
            .finish()
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl FromStr for PublicKeyDocument {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_public_key_pem(s)
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl pem::PemLabel for PublicKeyDocument {
    const TYPE_LABEL: &'static str = "PUBLIC KEY";
}

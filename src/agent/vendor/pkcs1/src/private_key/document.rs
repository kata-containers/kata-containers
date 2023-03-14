//! PKCS#1 RSA private key document.

use crate::{DecodeRsaPrivateKey, EncodeRsaPrivateKey, Error, Result, RsaPrivateKey};
use alloc::vec::Vec;
use core::fmt;
use der::{Decodable, Document, Encodable};
use zeroize::{Zeroize, Zeroizing};

#[cfg(feature = "pem")]
use {
    crate::{pem, LineEnding},
    alloc::string::String,
    core::str::FromStr,
};

#[cfg(feature = "std")]
use std::{path::Path, str};

/// PKCS#1 `RSA PRIVATE KEY` document.
///
/// This type provides storage for [`RsaPrivateKey`] encoded as ASN.1 DER
/// with the invariant that the contained-document is "well-formed", i.e. it
/// will parse successfully according to this crate's parsing rules.
#[derive(Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct RsaPrivateKeyDocument(Zeroizing<Vec<u8>>);

impl<'a> Document<'a> for RsaPrivateKeyDocument {
    type Message = RsaPrivateKey<'a>;
    const SENSITIVE: bool = true;
}

impl DecodeRsaPrivateKey for RsaPrivateKeyDocument {
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

impl EncodeRsaPrivateKey for RsaPrivateKeyDocument {
    fn to_pkcs1_der(&self) -> Result<RsaPrivateKeyDocument> {
        Ok(self.clone())
    }

    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn to_pkcs1_pem(&self, line_ending: LineEnding) -> Result<Zeroizing<String>> {
        Ok(Zeroizing::new(self.to_pem(line_ending)?))
    }

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_pkcs1_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(self.write_der_file(path)?)
    }

    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_pkcs1_pem_file(&self, path: impl AsRef<Path>, line_ending: LineEnding) -> Result<()> {
        Ok(self.write_pem_file(path, line_ending)?)
    }
}

impl AsRef<[u8]> for RsaPrivateKeyDocument {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl TryFrom<&[u8]> for RsaPrivateKeyDocument {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        RsaPrivateKeyDocument::from_pkcs1_der(bytes)
    }
}

impl TryFrom<RsaPrivateKey<'_>> for RsaPrivateKeyDocument {
    type Error = Error;

    fn try_from(private_key: RsaPrivateKey<'_>) -> Result<RsaPrivateKeyDocument> {
        RsaPrivateKeyDocument::try_from(&private_key)
    }
}

impl TryFrom<&RsaPrivateKey<'_>> for RsaPrivateKeyDocument {
    type Error = Error;

    fn try_from(private_key: &RsaPrivateKey<'_>) -> Result<RsaPrivateKeyDocument> {
        Ok(private_key.to_vec()?.try_into()?)
    }
}

impl TryFrom<Vec<u8>> for RsaPrivateKeyDocument {
    type Error = der::Error;

    fn try_from(mut bytes: Vec<u8>) -> der::Result<Self> {
        // Ensure document is well-formed
        if let Err(err) = RsaPrivateKey::from_der(bytes.as_slice()) {
            bytes.zeroize();
            return Err(err);
        }

        Ok(Self(Zeroizing::new(bytes)))
    }
}

impl fmt::Debug for RsaPrivateKeyDocument {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("RsaPrivateKeyDocument")
            .field(&self.decode())
            .finish()
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl FromStr for RsaPrivateKeyDocument {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_pkcs1_pem(s)
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl pem::PemLabel for RsaPrivateKeyDocument {
    const TYPE_LABEL: &'static str = "RSA PRIVATE KEY";
}

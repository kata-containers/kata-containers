//! PKCS#8 encrypted private key document.

use crate::{EncryptedPrivateKeyInfo, Error, Result};
use alloc::{borrow::ToOwned, vec::Vec};
use core::fmt;
use der::{Decodable, Document, Encodable};
use zeroize::{Zeroize, Zeroizing};

#[cfg(feature = "encryption")]
use crate::PrivateKeyDocument;

#[cfg(feature = "pem")]
use {core::str::FromStr, der::pem};

/// Encrypted PKCS#8 private key document.
///
/// This type provides heap-backed storage for [`EncryptedPrivateKeyInfo`]
/// encoded as ASN.1 DER with the invariant that the contained-document is
/// "well-formed", i.e. it will parse successfully according to this crate's
/// parsing rules.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(docsrs, doc(cfg(all(feature = "alloc", feature = "pkcs5"))))]
pub struct EncryptedPrivateKeyDocument(Zeroizing<Vec<u8>>);

impl<'a> Document<'a> for EncryptedPrivateKeyDocument {
    type Message = EncryptedPrivateKeyInfo<'a>;
    const SENSITIVE: bool = true;
}

impl EncryptedPrivateKeyDocument {
    /// Attempt to decrypt this encrypted private key using the provided
    /// password to derive an encryption key.
    #[cfg(feature = "encryption")]
    #[cfg_attr(docsrs, doc(cfg(feature = "encryption")))]
    pub fn decrypt(&self, password: impl AsRef<[u8]>) -> Result<PrivateKeyDocument> {
        self.encrypted_private_key_info().decrypt(password)
    }

    /// Parse the [`EncryptedPrivateKeyInfo`] contained in this [`EncryptedPrivateKeyDocument`].
    pub fn encrypted_private_key_info(&self) -> EncryptedPrivateKeyInfo<'_> {
        EncryptedPrivateKeyInfo::try_from(self.0.as_ref())
            .expect("malformed EncryptedPrivateKeyDocument")
    }
}

impl AsRef<[u8]> for EncryptedPrivateKeyDocument {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl TryFrom<EncryptedPrivateKeyInfo<'_>> for EncryptedPrivateKeyDocument {
    type Error = Error;

    fn try_from(key: EncryptedPrivateKeyInfo<'_>) -> Result<EncryptedPrivateKeyDocument> {
        EncryptedPrivateKeyDocument::try_from(&key)
    }
}

impl TryFrom<&EncryptedPrivateKeyInfo<'_>> for EncryptedPrivateKeyDocument {
    type Error = Error;

    fn try_from(key: &EncryptedPrivateKeyInfo<'_>) -> Result<EncryptedPrivateKeyDocument> {
        Ok(key.to_vec()?.try_into()?)
    }
}

impl TryFrom<&[u8]> for EncryptedPrivateKeyDocument {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        // Ensure document is well-formed
        EncryptedPrivateKeyInfo::try_from(bytes)?;
        Ok(Self(Zeroizing::new(bytes.to_owned())))
    }
}

impl TryFrom<Vec<u8>> for EncryptedPrivateKeyDocument {
    type Error = der::Error;

    fn try_from(mut bytes: Vec<u8>) -> der::Result<Self> {
        // Ensure document is well-formed
        if let Err(err) = EncryptedPrivateKeyInfo::from_der(bytes.as_slice()) {
            bytes.zeroize();
            return Err(err);
        }

        Ok(Self(Zeroizing::new(bytes)))
    }
}

impl fmt::Debug for EncryptedPrivateKeyDocument {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("EncryptedPrivateKeyDocument")
            .field(&self.encrypted_private_key_info())
            .finish()
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl FromStr for EncryptedPrivateKeyDocument {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Self::from_pem(s)?)
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl pem::PemLabel for EncryptedPrivateKeyDocument {
    const TYPE_LABEL: &'static str = "ENCRYPTED PRIVATE KEY";
}

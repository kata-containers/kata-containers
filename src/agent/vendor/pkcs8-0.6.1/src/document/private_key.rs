//! PKCS#8 private key document.

use crate::{error, Error, PrivateKeyInfo, Result};
use alloc::{borrow::ToOwned, vec::Vec};
use core::{
    convert::{TryFrom, TryInto},
    fmt,
};
use der::Encodable;
use zeroize::{Zeroize, Zeroizing};

#[cfg(feature = "encryption")]
use {
    crate::{EncryptedPrivateKeyDocument, EncryptedPrivateKeyInfo},
    pkcs5::pbes2,
    rand_core::{CryptoRng, RngCore},
};

#[cfg(feature = "pem")]
use {crate::pem, alloc::string::String, core::str::FromStr};

#[cfg(feature = "std")]
use std::{fs, path::Path, str};

/// PKCS#8 private key document.
///
/// This type provides storage for [`PrivateKeyInfo`] encoded as ASN.1 DER
/// with the invariant that the contained-document is "well-formed", i.e. it
/// will parse successfully according to this crate's parsing rules.
#[derive(Clone)]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct PrivateKeyDocument(Zeroizing<Vec<u8>>);

impl PrivateKeyDocument {
    /// Parse the [`PrivateKeyInfo`] contained in this [`PrivateKeyDocument`]
    pub fn private_key_info(&self) -> PrivateKeyInfo<'_> {
        PrivateKeyInfo::try_from(self.0.as_ref()).expect("malformed PrivateKeyDocument")
    }

    /// Parse [`PrivateKeyDocument`] from ASN.1 DER-encoded PKCS#8
    pub fn from_der(bytes: &[u8]) -> Result<Self> {
        bytes.try_into()
    }

    /// Parse [`PrivateKeyDocument`] from PEM-encoded PKCS#8.
    ///
    /// PEM-encoded private keys can be identified by the leading delimiter:
    ///
    /// ```text
    /// -----BEGIN PRIVATE KEY-----
    /// ```
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    pub fn from_pem(s: &str) -> Result<Self> {
        let der_bytes = pem::decode(s, pem::PRIVATE_KEY_BOUNDARY)?;
        Self::from_der(&*der_bytes)
    }

    /// Serialize [`PrivateKeyDocument`] as self-zeroizing PEM-encoded PKCS#8 string.
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    pub fn to_pem(&self) -> Zeroizing<String> {
        Zeroizing::new(pem::encode(&self.0, pem::PRIVATE_KEY_BOUNDARY))
    }

    /// Load [`PrivateKeyDocument`] from an ASN.1 DER-encoded file on the local
    /// filesystem (binary format).
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn read_der_file(path: impl AsRef<Path>) -> Result<Self> {
        fs::read(path)?.try_into()
    }

    /// Load [`PrivateKeyDocument`] from a PEM-encoded file on the local filesystem.
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn read_pem_file(path: impl AsRef<Path>) -> Result<Self> {
        Self::from_pem(&Zeroizing::new(fs::read_to_string(path)?))
    }

    /// Write ASN.1 DER-encoded PKCS#8 private key to the given path
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn write_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        write_secret_file(path, self.as_ref())
    }

    /// Write PEM-encoded PKCS#8 private key to the given path
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    pub fn write_pem_file(&self, path: impl AsRef<Path>) -> Result<()> {
        write_secret_file(path, self.to_pem().as_bytes())
    }

    /// Encrypt this private key using a symmetric encryption key derived
    /// from the provided password.
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

        let pbkdf2_iterations = 10_000;
        let pbes2_params =
            pbes2::Parameters::pbkdf2_sha256_aes256cbc(pbkdf2_iterations, &salt, &iv)
                .map_err(|_| Error::Crypto)?;

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
        pbes2_params
            .encrypt(password, self.as_ref())
            .map(|encrypted_data| {
                EncryptedPrivateKeyInfo {
                    encryption_algorithm: pbes2_params.into(),
                    encrypted_data: &encrypted_data,
                }
                .into()
            })
            .map_err(|_| Error::Encode)
    }
}

impl AsRef<[u8]> for PrivateKeyDocument {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<PrivateKeyInfo<'_>> for PrivateKeyDocument {
    fn from(private_key_info: PrivateKeyInfo<'_>) -> PrivateKeyDocument {
        PrivateKeyDocument::from(&private_key_info)
    }
}

impl From<&PrivateKeyInfo<'_>> for PrivateKeyDocument {
    fn from(private_key_info: &PrivateKeyInfo<'_>) -> PrivateKeyDocument {
        private_key_info
            .to_vec()
            .ok()
            .and_then(|buf| buf.try_into().ok())
            .expect(error::DER_ENCODING_MSG)
    }
}

impl TryFrom<&[u8]> for PrivateKeyDocument {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        // Ensure document is well-formed
        PrivateKeyInfo::try_from(bytes)?;
        Ok(Self(Zeroizing::new(bytes.to_owned())))
    }
}

impl TryFrom<Vec<u8>> for PrivateKeyDocument {
    type Error = Error;

    fn try_from(mut bytes: Vec<u8>) -> Result<Self> {
        // Ensure document is well-formed
        if PrivateKeyInfo::try_from(bytes.as_slice()).is_ok() {
            Ok(Self(Zeroizing::new(bytes)))
        } else {
            bytes.zeroize();
            Err(Error::Decode)
        }
    }
}

impl fmt::Debug for PrivateKeyDocument {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("PrivateKeyDocument")
            .field(&self.private_key_info())
            .finish()
    }
}

#[cfg(feature = "pem")]
#[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
impl FromStr for PrivateKeyDocument {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_pem(s)
    }
}

/// Write a file containing secret data to the filesystem, restricting the
/// file permissions so it's only readable by the owner
#[cfg(all(unix, feature = "std"))]
pub(super) fn write_secret_file(path: impl AsRef<Path>, data: &[u8]) -> Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};

    /// File permissions for secret data
    #[cfg(unix)]
    const SECRET_FILE_PERMS: u32 = 0o600;

    fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(SECRET_FILE_PERMS)
        .open(path)
        .and_then(|mut file| file.write_all(data))?;

    Ok(())
}

/// Write a file containing secret data to the filesystem
// TODO(tarcieri): permissions hardening on Windows
#[cfg(all(not(unix), feature = "std"))]
pub(super) fn write_secret_file(path: impl AsRef<Path>, data: &[u8]) -> Result<()> {
    fs::write(path, data)?;
    Ok(())
}

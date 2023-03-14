//! ASN.1 DER-encoded documents stored on the heap.

use crate::{Decodable, Encodable, Error, Result};
use alloc::{boxed::Box, vec::Vec};

#[cfg(feature = "pem")]
use {crate::pem, alloc::string::String};

#[cfg(feature = "std")]
use std::{fs, path::Path};

/// ASN.1 DER-encoded document.
///
/// This trait is intended to impl on types which contain an ASN.1 DER-encoded
/// document which is guaranteed to encode as the associated `Message` type.
///
/// It implements common functionality related to encoding/decoding such
/// documents, such as PEM encapsulation as well as reading/writing documents
/// from/to the filesystem.
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub trait Document<'a>: AsRef<[u8]> + Sized + TryFrom<Vec<u8>, Error = Error> {
    /// ASN.1 message type this document decodes to.
    type Message: Decodable<'a> + Encodable + Sized;

    /// Does this type contain potentially sensitive data?
    ///
    /// This enables hardened file permissions when persisting data to disk.
    const SENSITIVE: bool;

    /// Borrow the inner serialized bytes of this document.
    fn as_der(&self) -> &[u8] {
        self.as_ref()
    }

    /// Return an allocated ASN.1 DER serialization as a boxed slice.
    fn to_der(&self) -> Box<[u8]> {
        self.as_ref().to_vec().into_boxed_slice()
    }

    /// Decode this document as ASN.1 DER.
    fn decode(&'a self) -> Self::Message {
        Self::Message::from_der(self.as_ref()).expect("ASN.1 DER document malformed")
    }

    /// Create a new document from the provided ASN.1 DER bytes.
    fn from_der(bytes: &[u8]) -> Result<Self> {
        bytes.to_vec().try_into()
    }

    /// Encode the provided type as ASN.1 DER.
    fn from_msg(msg: &Self::Message) -> Result<Self> {
        msg.to_vec()?.try_into()
    }

    /// Decode ASN.1 DER document from PEM.
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn from_pem(s: &str) -> Result<Self>
    where
        Self: pem::PemLabel,
    {
        let (label, der_bytes) = pem::decode_vec(s.as_bytes())?;

        if label != Self::TYPE_LABEL {
            return Err(pem::Error::Label.into());
        }

        der_bytes.try_into()
    }

    /// Encode ASN.1 DER document as a PEM string.
    #[cfg(feature = "pem")]
    #[cfg_attr(docsrs, doc(cfg(feature = "pem")))]
    fn to_pem(&self, line_ending: pem::LineEnding) -> Result<String>
    where
        Self: pem::PemLabel,
    {
        Ok(pem::encode_string(
            Self::TYPE_LABEL,
            line_ending,
            self.as_ref(),
        )?)
    }

    /// Read ASN.1 DER document from a file.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn read_der_file(path: impl AsRef<Path>) -> Result<Self> {
        fs::read(path)?.try_into()
    }

    /// Read PEM-encoded ASN.1 DER document from a file.
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "pem", feature = "std"))))]
    fn read_pem_file(path: impl AsRef<Path>) -> Result<Self>
    where
        Self: pem::PemLabel,
    {
        Self::from_pem(&fs::read_to_string(path)?)
    }

    /// Write ASN.1 DER document to a file.
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn write_der_file(&self, path: impl AsRef<Path>) -> Result<()> {
        write_file(path, self.as_ref(), Self::SENSITIVE)
    }

    /// Write PEM-encoded ASN.1 DER document to a file.
    #[cfg(all(feature = "pem", feature = "std"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "pem", feature = "std"))))]
    fn write_pem_file(&self, path: impl AsRef<Path>, line_ending: pem::LineEnding) -> Result<()>
    where
        Self: pem::PemLabel,
    {
        write_file(path, self.to_pem(line_ending)?.as_bytes(), Self::SENSITIVE)
    }
}

/// Write a file to the filesystem, potentially using hardened permissions
/// if the file contains secret data.
#[cfg(feature = "std")]
fn write_file(path: impl AsRef<Path>, data: &[u8], sensitive: bool) -> Result<()> {
    if sensitive {
        write_secret_file(path, data)
    } else {
        Ok(fs::write(path, data)?)
    }
}

/// Write a file containing secret data to the filesystem, restricting the
/// file permissions so it's only readable by the owner
#[cfg(all(unix, feature = "std"))]
fn write_secret_file(path: impl AsRef<Path>, data: &[u8]) -> Result<()> {
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
fn write_secret_file(path: impl AsRef<Path>, data: &[u8]) -> Result<()> {
    fs::write(path, data)?;
    Ok(())
}

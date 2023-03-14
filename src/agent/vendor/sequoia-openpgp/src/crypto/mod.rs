//! Cryptographic primitives.
//!
//! This module contains cryptographic primitives as defined and used
//! by OpenPGP.  It abstracts over the cryptographic library chosen at
//! compile time.  Most of the time, it will not be necessary to
//! explicitly use types from this module directly, but they are used
//! in the API (e.g. [`Password`]).  Advanced users may use these
//! primitives to provide custom extensions to OpenPGP.
//!
//!
//! # Common Operations
//!
//!  - *Converting a string to a [`Password`]*: Use [`Password::from`].
//!  - *Create a session key*: Use [`SessionKey::new`].
//!  - *Use secret keys*: See the [`KeyPair` example].
//!
//!   [`Password::from`]: std::convert::From
//!   [`SessionKey::new`]: SessionKey::new()
//!   [`KeyPair` example]: KeyPair#examples

use std::cmp::Ordering;
use std::ops::{Deref, DerefMut};
use std::fmt;
use std::borrow::Cow;

use crate::{
    Error,
    Result,
};

pub(crate) mod aead;
mod asymmetric;
pub use self::asymmetric::{Signer, Decryptor, KeyPair};
mod backend;
pub mod ecdh;
pub mod hash;
pub mod mem;
pub mod mpi;
mod s2k;
pub use s2k::S2K;
pub(crate) mod symmetric;

#[cfg(test)]
mod tests;

/// Returns a short, human-readable description of the backend.
///
/// This starts with the name of the backend, possibly a version, and
/// any optional features that are available.  This is meant for
/// inclusion in version strings to improve bug reports.
pub fn backend() -> String {
    backend::backend()
}

/// Fills the given buffer with random data.
///
/// Fills the given buffer with random data produced by a
/// cryptographically secure pseudorandom number generator (CSPRNG).
/// The output may be used as session keys or to derive long-term
/// cryptographic keys from.  However, to create session keys,
/// consider using [`SessionKey::new`].
///
///   [`SessionKey::new`]: crate::crypto::SessionKey::new()
pub fn random<B: AsMut<[u8]>>(mut buf: B) {
    backend::random(buf.as_mut());
}

/// Holds a session key.
///
/// The session key is cleared when dropped.  Sequoia uses this type
/// to ensure that session keys are not left in memory returned to the
/// allocator.
///
/// Session keys can be generated using [`SessionKey::new`], or
/// converted from various types using [`From`].
///
///   [`SessionKey::new`]: SessionKey::new()
///   [`From`]: std::convert::From
#[derive(Clone, PartialEq, Eq)]
pub struct SessionKey(mem::Protected);
assert_send_and_sync!(SessionKey);

impl SessionKey {
    /// Creates a new session key.
    ///
    /// Creates a new session key `size` bytes in length initialized
    /// using a strong cryptographic number generator.
    ///
    /// # Examples
    ///
    /// This creates a session key and encrypts it for a given
    /// recipient key producing a [`PKESK`] packet.
    ///
    ///   [`PKESK`]: crate::packet::PKESK
    ///
    /// ```
    /// # fn main() -> sequoia_openpgp::Result<()> {
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::types::{Curve, SymmetricAlgorithm};
    /// use openpgp::crypto::SessionKey;
    /// use openpgp::packet::prelude::*;
    ///
    /// let cipher = SymmetricAlgorithm::AES256;
    /// let sk = SessionKey::new(cipher.key_size().unwrap());
    ///
    /// let key: Key<key::SecretParts, key::UnspecifiedRole> =
    ///     Key4::generate_ecc(false, Curve::Cv25519)?.into();
    ///
    /// let pkesk: PKESK =
    ///     PKESK3::for_recipient(cipher, &sk, &key)?.into();
    /// # Ok(()) }
    /// ```
    pub fn new(size: usize) -> Self {
        let mut sk: mem::Protected = vec![0; size].into();
        random(&mut sk);
        Self(sk)
    }
}

impl Deref for SessionKey {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<[u8]> for SessionKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl DerefMut for SessionKey {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl AsMut<[u8]> for SessionKey {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl From<mem::Protected> for SessionKey {
    fn from(v: mem::Protected) -> Self {
        SessionKey(v)
    }
}

impl From<Vec<u8>> for SessionKey {
    fn from(v: Vec<u8>) -> Self {
        SessionKey(v.into())
    }
}

impl From<Box<[u8]>> for SessionKey {
    fn from(v: Box<[u8]>) -> Self {
        SessionKey(v.into())
    }
}

impl From<&[u8]> for SessionKey {
    fn from(v: &[u8]) -> Self {
        Vec::from(v).into()
    }
}

impl fmt::Debug for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SessionKey ({:?})", self.0)
    }
}

/// Holds a password.
///
/// `Password`s can be converted from various types using [`From`].
/// The password is encrypted in memory and only decrypted on demand.
/// See [`mem::Encrypted`] for details.
///
///   [`From`]: std::convert::From
///
/// # Examples
///
/// ```
/// use sequoia_openpgp as openpgp;
/// use openpgp::crypto::Password;
///
/// // Convert from a &str.
/// let p: Password = "hunter2".into();
///
/// // Convert from a &[u8].
/// let p: Password = b"hunter2"[..].into();
///
/// // Convert from a String.
/// let p: Password = String::from("hunter2").into();
///
/// // ...
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Password(mem::Encrypted);
assert_send_and_sync!(Password);

impl From<Vec<u8>> for Password {
    fn from(v: Vec<u8>) -> Self {
        Password(mem::Encrypted::new(v.into()))
    }
}

impl From<Box<[u8]>> for Password {
    fn from(v: Box<[u8]>) -> Self {
        Password(mem::Encrypted::new(v.into()))
    }
}

impl From<String> for Password {
    fn from(v: String) -> Self {
        v.into_bytes().into()
    }
}

impl<'a> From<&'a str> for Password {
    fn from(v: &'a str) -> Self {
        v.to_owned().into()
    }
}

impl From<&[u8]> for Password {
    fn from(v: &[u8]) -> Self {
        Vec::from(v).into()
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if cfg!(debug_assertions) {
            self.map(|p| write!(f, "Password({:?})", p))
        } else {
            f.write_str("Password(<Encrypted>)")
        }
    }
}

impl Password {
    /// Maps the given function over the password.
    ///
    /// The password is stored encrypted in memory.  This function
    /// temporarily decrypts it for the given function to use.
    ///
    /// # Examples
    ///
    /// ```
    /// use sequoia_openpgp as openpgp;
    /// use openpgp::crypto::Password;
    ///
    /// let p: Password = "hunter2".into();
    /// p.map(|p| assert_eq!(p.as_ref(), &b"hunter2"[..]));
    /// ```
    pub fn map<F, T>(&self, fun: F) -> T
        where F: FnMut(&mem::Protected) -> T
    {
        self.0.map(fun)
    }
}

/// Returns the value zero-padded to the given length.
///
/// Some encodings strip leading zero-bytes.  This function adds them
/// back, if necessary.  If the size exceeds `to`, an error is
/// returned.
pub(crate) fn pad(value: &[u8], to: usize) -> Result<Cow<[u8]>>
{
    match value.len().cmp(&to) {
        Ordering::Equal => Ok(Cow::Borrowed(value)),
        Ordering::Less => {
            let missing = to - value.len();
            let mut v = vec![0; to];
            v[missing..].copy_from_slice(value);
            Ok(Cow::Owned(v))
        }
        Ordering::Greater => {
            Err(Error::InvalidOperation(
                format!("Input value is longer than expected: {} > {}",
                        value.len(), to)).into())
        }
    }
}

/// Returns the value zero-padded to the given length.
///
/// Some encodings strip leading zero-bytes.  This function adds them
/// back, if necessary.  If the size exceeds `to`, the value is
/// returned as-is.
#[allow(dead_code)]
#[allow(clippy::unnecessary_lazy_evaluations)]
pub(crate) fn pad_at_least(value: &[u8], to: usize) -> Cow<[u8]>
{
    pad(value, to).unwrap_or(Cow::Borrowed(value))
}

/// Returns the value zero-padded or truncated to the given length.
///
/// Some encodings strip leading zero-bytes.  This function adds them
/// back, if necessary.  If the size exceeds `to`, the value is
/// silently truncated.
#[allow(dead_code)]
pub(crate) fn pad_truncating(value: &[u8], to: usize) -> Cow<[u8]>
{
    if value.len() == to {
        Cow::Borrowed(value)
    } else {
        let missing = to.saturating_sub(value.len());
        let limit = value.len().min(to);
        let mut v = vec![0; to];
        v[missing..].copy_from_slice(&value[..limit]);
        Cow::Owned(v)
    }
}

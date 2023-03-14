//! [Authenticated Encryption with Associated Data] (AEAD) traits
//!
//! This crate provides an abstract interface for AEAD ciphers, which guarantee
//! both confidentiality and integrity, even from a powerful attacker who is
//! able to execute [chosen-ciphertext attacks]. The resulting security property,
//! [ciphertext indistinguishability], is considered a basic requirement for
//! modern cryptographic implementations.
//!
//! See [RustCrypto/AEADs] for cipher implementations which use this trait.
//!
//! [Authenticated Encryption with Associated Data]: https://en.wikipedia.org/wiki/Authenticated_encryption
//! [chosen-ciphertext attacks]: https://en.wikipedia.org/wiki/Chosen-ciphertext_attack
//! [ciphertext indistinguishability]: https://en.wikipedia.org/wiki/Ciphertext_indistinguishability
//! [RustCrypto/AEADs]: https://github.com/RustCrypto/AEADs

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![forbid(unsafe_code)]
#![doc(html_logo_url = "https://raw.githubusercontent.com/RustCrypto/meta/master/logo_small.png")]
#![warn(missing_docs, rust_2018_idioms)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "dev")]
#[cfg_attr(docsrs, doc(cfg(feature = "dev")))]
pub mod dev;

pub use generic_array::{self, typenum::consts};

#[cfg(feature = "heapless")]
pub use heapless;

use core::fmt;
use generic_array::{typenum::Unsigned, ArrayLength, GenericArray};

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// Error type.
///
/// This type is deliberately opaque as to avoid potential side-channel
/// leakage (e.g. padding oracle).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Error;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("aead::Error")
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// Key for a [`NewAead`] algorithm
// TODO(tarcieri): make this a struct and zeroize on drop?
pub type Key<A> = GenericArray<u8, <A as NewAead>::KeySize>;

/// Nonce: single-use value for ensuring ciphertexts are unique
pub type Nonce<NonceSize> = GenericArray<u8, NonceSize>;

/// Tag: authentication code which ensures ciphertexts are authentic
pub type Tag<TagSize> = GenericArray<u8, TagSize>;

/// Instantiate either a stateless [`Aead`] or stateful [`AeadMut`] algorithm.
pub trait NewAead {
    /// The size of the key array required by this algorithm.
    type KeySize: ArrayLength<u8>;

    /// Create a new AEAD instance with the given key.
    fn new(key: &Key<Self>) -> Self;

    /// Create new AEAD instance from key with variable size.
    ///
    /// Default implementation will accept only keys with length equal to `KeySize`.
    fn new_varkey(key: &[u8]) -> Result<Self, Error>
    where
        Self: Sized,
    {
        if key.len() != Self::KeySize::to_usize() {
            Err(Error)
        } else {
            Ok(Self::new(GenericArray::from_slice(key)))
        }
    }
}

/// Authenticated Encryption with Associated Data (AEAD) algorithm.
///
/// This trait is intended for use with stateless AEAD algorithms. The
/// [`AeadMut`] trait provides a stateful interface.
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub trait Aead {
    /// The length of a nonce.
    type NonceSize: ArrayLength<u8>;

    /// The maximum length of the nonce.
    type TagSize: ArrayLength<u8>;

    /// The upper bound amount of additional space required to support a
    /// ciphertext vs. a plaintext.
    type CiphertextOverhead: ArrayLength<u8> + Unsigned;

    /// Encrypt the given plaintext payload, and return the resulting
    /// ciphertext as a vector of bytes.
    ///
    /// The [`Payload`] type can be used to provide Additional Associated Data
    /// (AAD) along with the message: this is an optional bytestring which is
    /// not encrypted, but *is* authenticated along with the message. Failure
    /// to pass the same AAD that was used during encryption will cause
    /// decryption to fail, which is useful if you would like to "bind" the
    /// ciphertext to some other identifier, like a digital signature key
    /// or other identifier.
    ///
    /// If you don't care about AAD and just want to encrypt a plaintext
    /// message, `&[u8]` will automatically be coerced into a `Payload`:
    ///
    /// ```nobuild
    /// let plaintext = b"Top secret message, handle with care";
    /// let ciphertext = cipher.encrypt(nonce, plaintext);
    /// ```
    ///
    /// The default implementation assumes a postfix tag (ala AES-GCM,
    /// AES-GCM-SIV, ChaCha20Poly1305). [`Aead`] implementations which do not
    /// use a postfix tag will need to override this to correctly assemble the
    /// ciphertext message.
    fn encrypt<'msg, 'aad>(
        &self,
        nonce: &Nonce<Self::NonceSize>,
        plaintext: impl Into<Payload<'msg, 'aad>>,
    ) -> Result<Vec<u8>, Error>;

    /// Decrypt the given ciphertext slice, and return the resulting plaintext
    /// as a vector of bytes.
    ///
    /// See notes on [`Aead::encrypt()`] about allowable message payloads and
    /// Associated Additional Data (AAD).
    ///
    /// If you have no AAD, you can call this as follows:
    ///
    /// ```nobuild
    /// let ciphertext = b"...";
    /// let plaintext = cipher.decrypt(nonce, ciphertext)?;
    /// ```
    ///
    /// The default implementation assumes a postfix tag (ala AES-GCM,
    /// AES-GCM-SIV, ChaCha20Poly1305). [`Aead`] implementations which do not
    /// use a postfix tag will need to override this to correctly parse the
    /// ciphertext message.
    fn decrypt<'msg, 'aad>(
        &self,
        nonce: &Nonce<Self::NonceSize>,
        ciphertext: impl Into<Payload<'msg, 'aad>>,
    ) -> Result<Vec<u8>, Error>;
}

/// Stateful Authenticated Encryption with Associated Data algorithm.
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub trait AeadMut {
    /// The length of a nonce.
    type NonceSize: ArrayLength<u8>;

    /// The maximum length of the nonce.
    type TagSize: ArrayLength<u8>;

    /// The upper bound amount of additional space required to support a
    /// ciphertext vs. a plaintext.
    type CiphertextOverhead: ArrayLength<u8> + Unsigned;

    /// Encrypt the given plaintext slice, and return the resulting ciphertext
    /// as a vector of bytes.
    ///
    /// See notes on [`Aead::encrypt()`] about allowable message payloads and
    /// Associated Additional Data (AAD).
    fn encrypt<'msg, 'aad>(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        plaintext: impl Into<Payload<'msg, 'aad>>,
    ) -> Result<Vec<u8>, Error>;

    /// Decrypt the given ciphertext slice, and return the resulting plaintext
    /// as a vector of bytes.
    ///
    /// See notes on [`Aead::encrypt()`] and [`Aead::decrypt()`] about allowable
    /// message payloads and Associated Additional Data (AAD).
    fn decrypt<'msg, 'aad>(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        ciphertext: impl Into<Payload<'msg, 'aad>>,
    ) -> Result<Vec<u8>, Error>;
}

/// Implement the `decrypt_in_place` method on [`AeadInPlace`] and
/// [`AeadMutInPlace]`, using a macro to gloss over the `&self` vs `&mut self`.
///
/// Assumes a postfix authentication tag. AEAD ciphers which do not use a
/// postfix authentication tag will need to define their own implementation.
macro_rules! impl_decrypt_in_place {
    ($aead:expr, $nonce:expr, $aad:expr, $buffer:expr) => {{
        if $buffer.len() < Self::TagSize::to_usize() {
            return Err(Error);
        }

        let tag_pos = $buffer.len() - Self::TagSize::to_usize();
        let (msg, tag) = $buffer.as_mut().split_at_mut(tag_pos);
        $aead.decrypt_in_place_detached($nonce, $aad, msg, Tag::from_slice(tag))?;
        $buffer.truncate(tag_pos);
        Ok(())
    }};
}

/// In-place stateless AEAD trait.
///
/// This trait is both object safe and has no dependencies on `alloc` or `std`.
pub trait AeadInPlace {
    /// The length of a nonce.
    type NonceSize: ArrayLength<u8>;

    /// The maximum length of the nonce.
    type TagSize: ArrayLength<u8>;

    /// The upper bound amount of additional space required to support a
    /// ciphertext vs. a plaintext.
    type CiphertextOverhead: ArrayLength<u8> + Unsigned;

    /// Encrypt the given buffer containing a plaintext message in-place.
    ///
    /// The buffer must have sufficient capacity to store the ciphertext
    /// message, which will always be larger than the original plaintext.
    /// The exact size needed is cipher-dependent, but generally includes
    /// the size of an authentication tag.
    ///
    /// Returns an error if the buffer has insufficient capacity to store the
    /// resulting ciphertext message.
    fn encrypt_in_place(
        &self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut dyn Buffer,
    ) -> Result<(), Error> {
        let tag = self.encrypt_in_place_detached(nonce, associated_data, buffer.as_mut())?;
        buffer.extend_from_slice(tag.as_slice())?;
        Ok(())
    }

    /// Encrypt the data in-place, returning the authentication tag
    fn encrypt_in_place_detached(
        &self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut [u8],
    ) -> Result<Tag<Self::TagSize>, Error>;

    /// Decrypt the message in-place, returning an error in the event the
    /// provided authentication tag does not match the given ciphertext.
    ///
    /// The buffer will be truncated to the length of the original plaintext
    /// message upon success.
    fn decrypt_in_place(
        &self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut dyn Buffer,
    ) -> Result<(), Error> {
        impl_decrypt_in_place!(self, nonce, associated_data, buffer)
    }

    /// Decrypt the message in-place, returning an error in the event the provided
    /// authentication tag does not match the given ciphertext (i.e. ciphertext
    /// is modified/unauthentic)
    fn decrypt_in_place_detached(
        &self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut [u8],
        tag: &Tag<Self::TagSize>,
    ) -> Result<(), Error>;
}

/// In-place stateful AEAD trait.
///
/// This trait is both object safe and has no dependencies on `alloc` or `std`.
pub trait AeadMutInPlace {
    /// The length of a nonce.
    type NonceSize: ArrayLength<u8>;

    /// The maximum length of the nonce.
    type TagSize: ArrayLength<u8>;

    /// The upper bound amount of additional space required to support a
    /// ciphertext vs. a plaintext.
    type CiphertextOverhead: ArrayLength<u8> + Unsigned;

    /// Encrypt the given buffer containing a plaintext message in-place.
    ///
    /// The buffer must have sufficient capacity to store the ciphertext
    /// message, which will always be larger than the original plaintext.
    /// The exact size needed is cipher-dependent, but generally includes
    /// the size of an authentication tag.
    ///
    /// Returns an error if the buffer has insufficient capacity to store the
    /// resulting ciphertext message.
    fn encrypt_in_place(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut impl Buffer,
    ) -> Result<(), Error> {
        let tag = self.encrypt_in_place_detached(nonce, associated_data, buffer.as_mut())?;
        buffer.extend_from_slice(tag.as_slice())?;
        Ok(())
    }

    /// Encrypt the data in-place, returning the authentication tag
    fn encrypt_in_place_detached(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut [u8],
    ) -> Result<Tag<Self::TagSize>, Error>;

    /// Decrypt the message in-place, returning an error in the event the
    /// provided authentication tag does not match the given ciphertext.
    ///
    /// The buffer will be truncated to the length of the original plaintext
    /// message upon success.
    fn decrypt_in_place(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut impl Buffer,
    ) -> Result<(), Error> {
        impl_decrypt_in_place!(self, nonce, associated_data, buffer)
    }

    /// Decrypt the data in-place, returning an error in the event the provided
    /// authentication tag does not match the given ciphertext (i.e. ciphertext
    /// is modified/unauthentic)
    fn decrypt_in_place_detached(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut [u8],
        tag: &Tag<Self::TagSize>,
    ) -> Result<(), Error>;
}

#[cfg(feature = "alloc")]
impl<Alg: AeadInPlace> Aead for Alg {
    type NonceSize = Alg::NonceSize;
    type TagSize = Alg::TagSize;
    type CiphertextOverhead = Alg::CiphertextOverhead;

    fn encrypt<'msg, 'aad>(
        &self,
        nonce: &Nonce<Self::NonceSize>,
        plaintext: impl Into<Payload<'msg, 'aad>>,
    ) -> Result<Vec<u8>, Error> {
        let payload = plaintext.into();
        let mut buffer = Vec::with_capacity(payload.msg.len() + Self::TagSize::to_usize());
        buffer.extend_from_slice(payload.msg);
        self.encrypt_in_place(nonce, payload.aad, &mut buffer)?;
        Ok(buffer)
    }

    fn decrypt<'msg, 'aad>(
        &self,
        nonce: &Nonce<Self::NonceSize>,
        ciphertext: impl Into<Payload<'msg, 'aad>>,
    ) -> Result<Vec<u8>, Error> {
        let payload = ciphertext.into();
        let mut buffer = Vec::from(payload.msg);
        self.decrypt_in_place(nonce, payload.aad, &mut buffer)?;
        Ok(buffer)
    }
}

#[cfg(feature = "alloc")]
impl<Alg: AeadMutInPlace> AeadMut for Alg {
    type NonceSize = Alg::NonceSize;
    type TagSize = Alg::TagSize;
    type CiphertextOverhead = Alg::CiphertextOverhead;

    fn encrypt<'msg, 'aad>(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        plaintext: impl Into<Payload<'msg, 'aad>>,
    ) -> Result<Vec<u8>, Error> {
        let payload = plaintext.into();
        let mut buffer = Vec::with_capacity(payload.msg.len() + Self::TagSize::to_usize());
        buffer.extend_from_slice(payload.msg);
        self.encrypt_in_place(nonce, payload.aad, &mut buffer)?;
        Ok(buffer)
    }

    fn decrypt<'msg, 'aad>(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        ciphertext: impl Into<Payload<'msg, 'aad>>,
    ) -> Result<Vec<u8>, Error> {
        let payload = ciphertext.into();
        let mut buffer = Vec::from(payload.msg);
        self.decrypt_in_place(nonce, payload.aad, &mut buffer)?;
        Ok(buffer)
    }
}

impl<Alg: AeadInPlace> AeadMutInPlace for Alg {
    type NonceSize = Alg::NonceSize;
    type TagSize = Alg::TagSize;
    type CiphertextOverhead = Alg::CiphertextOverhead;

    fn encrypt_in_place(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut impl Buffer,
    ) -> Result<(), Error> {
        <Self as AeadInPlace>::encrypt_in_place(self, nonce, associated_data, buffer)
    }

    fn encrypt_in_place_detached(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut [u8],
    ) -> Result<Tag<Self::TagSize>, Error> {
        <Self as AeadInPlace>::encrypt_in_place_detached(self, nonce, associated_data, buffer)
    }

    fn decrypt_in_place(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut impl Buffer,
    ) -> Result<(), Error> {
        <Self as AeadInPlace>::decrypt_in_place(self, nonce, associated_data, buffer)
    }

    fn decrypt_in_place_detached(
        &mut self,
        nonce: &Nonce<Self::NonceSize>,
        associated_data: &[u8],
        buffer: &mut [u8],
        tag: &Tag<Self::TagSize>,
    ) -> Result<(), Error> {
        <Self as AeadInPlace>::decrypt_in_place_detached(self, nonce, associated_data, buffer, tag)
    }
}

/// AEAD payloads are a combination of a message (plaintext or ciphertext)
/// and "additional associated data" (AAD) to be authenticated (in cleartext)
/// along with the message.
///
/// If you don't care about AAD, you can pass a `&[u8]` as the payload to
/// `encrypt`/`decrypt` and it will automatically be coerced to this type.
#[cfg(feature = "alloc")]
#[cfg_attr(docsrs, doc(cfg(feature = "alloc")))]
pub struct Payload<'msg, 'aad> {
    /// Message to be encrypted/decrypted
    pub msg: &'msg [u8],

    /// Optional "additional associated data" to authenticate along with
    /// this message. If AAD is provided at the time the message is encrypted,
    /// the same AAD *MUST* be provided at the time the message is decrypted,
    /// or decryption will fail.
    pub aad: &'aad [u8],
}

#[cfg(feature = "alloc")]
impl<'msg, 'aad> From<&'msg [u8]> for Payload<'msg, 'aad> {
    fn from(msg: &'msg [u8]) -> Self {
        Self { msg, aad: b"" }
    }
}

/// In-place encryption/decryption byte buffers.
///
/// This trait defines the set of methods needed to support in-place operations
/// on a `Vec`-like data type.
pub trait Buffer: AsRef<[u8]> + AsMut<[u8]> {
    /// Get the length of the buffer
    fn len(&self) -> usize {
        self.as_ref().len()
    }

    /// Is the buffer empty?
    fn is_empty(&self) -> bool {
        self.as_ref().is_empty()
    }

    /// Extend this buffer from the given slice
    fn extend_from_slice(&mut self, other: &[u8]) -> Result<(), Error>;

    /// Truncate this buffer to the given size
    fn truncate(&mut self, len: usize);
}

#[cfg(feature = "alloc")]
impl Buffer for Vec<u8> {
    fn extend_from_slice(&mut self, other: &[u8]) -> Result<(), Error> {
        Vec::extend_from_slice(self, other);
        Ok(())
    }

    fn truncate(&mut self, len: usize) {
        Vec::truncate(self, len);
    }
}

#[cfg(feature = "heapless")]
impl<N> Buffer for heapless::Vec<u8, N>
where
    N: heapless::ArrayLength<u8>,
{
    fn extend_from_slice(&mut self, other: &[u8]) -> Result<(), Error> {
        heapless::Vec::extend_from_slice(self, other).map_err(|_| Error)
    }

    fn truncate(&mut self, len: usize) {
        heapless::Vec::truncate(self, len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ensure that `AeadInPlace` is object-safe
    #[allow(dead_code)]
    type DynAeadInPlace<N, T, O> =
        dyn AeadInPlace<NonceSize = N, TagSize = T, CiphertextOverhead = O>;

    /// Ensure that `AeadMutInPlace` is object-safe
    #[allow(dead_code)]
    type DynAeadMutInPlace<N, T, O> =
        dyn AeadMutInPlace<NonceSize = N, TagSize = T, CiphertextOverhead = O>;
}

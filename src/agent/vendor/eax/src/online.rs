//! Online[<sup>1</sup>] variant of the EAX mode.
//!
//! # Authentication
//! Due to *AE* (authenticated encryption) nature of EAX, it is vital to verify
//! that both public (also called *associated*) and privacy-protected
//! (encrypted) data has not been tampered with.
//!
//! Because of this, it is required for the consumers to explicitly call
//! [`finish`] after the encryption/decryption operation is complete.
//! This will either return a *tag* (when encrypting) used to authenticate data
//! or a `Result` (when decrypting) that signifies whether the data is authentic,
//! which is when the resulting tag is equal to the one created during encryption.
//!
//! ## Example
//! ```
//! use eax::{Error, online::{Eax, Decrypt, Encrypt}, cipher::generic_array::GenericArray};
//! use aes::Aes256;
//!
//! let key = GenericArray::from_slice(b"an example very very secret key.");
//! let nonce = GenericArray::from_slice(b"my unique nonces"); // 128-bits; unique per message
//! let assoc = b"my associated data";
//! let plaintext = b"plaintext message";
//! let mut buffer: [u8; 17] = *plaintext;
//!
//!// Encrypt a simple message
//! let mut cipher = Eax::<Aes256, Encrypt>::with_key_and_nonce(key, nonce);
//! cipher.update_assoc(&assoc[..]);
//! cipher.encrypt(&mut buffer[..9]);
//! cipher.encrypt(&mut buffer[9..]);
//! let tag = cipher.finish();
//!
//! assert_ne!(buffer, *plaintext);
//!
//! let mut cloned = buffer;
//!
//! // Now decrypt it, using the same key and nonce
//! let mut cipher = Eax::<Aes256, Decrypt>::with_key_and_nonce(key, nonce);
//! cipher.update_assoc(&assoc[..]);
//! cipher.decrypt_unauthenticated_hazmat(&mut buffer[..5]);
//! cipher.decrypt_unauthenticated_hazmat(&mut buffer[5..10]);
//! cipher.decrypt_unauthenticated_hazmat(&mut buffer[10..]);
//! let res = cipher.finish(&tag);
//!
//! assert_eq!(res, Ok(()));
//! assert_eq!(buffer, *plaintext);
//!
//! // Decrypting the ciphertext with tampered associated data should fail
//! let mut cipher = Eax::<Aes256, Decrypt>::with_key_and_nonce(key, nonce);
//! cipher.update_assoc(b"tampered");
//! cipher.decrypt_unauthenticated_hazmat(&mut cloned);
//! let res = cipher.finish(&tag);
//!
//! assert_eq!(res, Err(Error));
//! ```
//! [<sup>1</sup>]: https://en.wikipedia.org/wiki/Online_algorithm
//! [`Eax`]: struct.Eax.html
//! [`Decrypt`]: struct.Decrypt.html
//! [`finish`]: #method.finish

use crate::*;

use core::marker::PhantomData;

pub use Eax as EaxOnline;

/// Marker trait denoting whether the EAX stream is used for encryption/decryption.
pub trait CipherOp {}
/// Marker struct for EAX stream used in encryption mode.
pub struct Encrypt;
impl CipherOp for Encrypt {}
/// Marker struct for EAX stream used in decryption mode.
pub struct Decrypt;
impl CipherOp for Decrypt {}

/// Online[<sup>1</sup>] variant of the EAX mode.
///
/// This type is generic to support substituting alternative cipher
/// implementations.
///
/// In contrast to [`Eax`], can be used in an online[<sup>1</sup>] fashion and
/// operates in-place.
///
/// # Authentication
/// Due to *AE* (authenticated encryption) nature of EAX, it is vital to verify
/// that both public (also called *associated*) and privacy-protected
/// (encrypted) data has not been tampered with.
///
/// Because of this, it is required for the consumers to explicitly call
/// [`finish`] after the encryption/decryption operation is complete.
/// This will either return a *tag* (when encrypting) used to authenticate data
/// or a `Result` (when decrypting) that signifies whether the data is authentic,
/// which is when the resulting tag is equal to the one created during encryption.
///
/// ## Example
/// ```
/// use eax::{Error, online::{Eax, Decrypt, Encrypt}, cipher::generic_array::GenericArray};
/// use aes::Aes256;
///
/// let key = GenericArray::from_slice(b"an example very very secret key.");
///
/// let nonce = GenericArray::from_slice(b"my unique nonces"); // 128-bits; unique per message
///
/// let assoc = b"my associated data";
/// let plaintext = b"plaintext message";
///
/// let mut buffer: [u8; 17] = *plaintext;
///
/// // Encrypt a simple message
/// let mut cipher = Eax::<Aes256, Encrypt>::with_key_and_nonce(key, nonce);
/// cipher.update_assoc(&assoc[..]);
/// cipher.encrypt(&mut buffer[..9]);
/// cipher.encrypt(&mut buffer[9..]);
/// let tag = cipher.finish();
///
/// assert_ne!(buffer, *plaintext);
///
/// let mut cloned = buffer;
///
/// // Now decrypt it, using the same key and nonce
/// let mut cipher = Eax::<Aes256, Decrypt>::with_key_and_nonce(key, nonce);
/// cipher.update_assoc(&assoc[..]);
/// cipher.decrypt_unauthenticated_hazmat(&mut buffer[..5]);
/// cipher.decrypt_unauthenticated_hazmat(&mut buffer[5..10]);
/// cipher.decrypt_unauthenticated_hazmat(&mut buffer[10..]);
/// let res = cipher.finish(&tag);
///
/// assert_eq!(res, Ok(()));
/// assert_eq!(buffer, *plaintext);
///
/// // Decrypting the ciphertext with tampered associated data should fail
/// let mut cipher = Eax::<Aes256, Decrypt>::with_key_and_nonce(key, nonce);
///
/// cipher.update_assoc(b"tampered");
/// cipher.decrypt_unauthenticated_hazmat(&mut cloned);
/// let res = cipher.finish(&tag);
///
/// assert_eq!(res, Err(Error));
/// ```
///
/// [<sup>1</sup>]: https://en.wikipedia.org/wiki/Online_algorithm
/// [`Eax`]: ../struct.Eax.html
/// [`Decrypt`]: struct.Decrypt.html
/// [`finish`]: #method.finish
pub struct Eax<Cipher, Op>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
    Op: CipherOp,
{
    imp: EaxImpl<Cipher>,
    /// Denotes whether this stream is used for encryption or decryption.
    marker: PhantomData<Op>,
}

impl<Cipher, Op> Eax<Cipher, Op>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
    Op: CipherOp,
{
    /// Creates a stateful EAX instance that is capable of processing both
    /// the associated data and the plaintext in an "on-line" fashion.
    pub fn with_key_and_nonce(key: &Key<Cipher>, nonce: &Nonce<Cipher::BlockSize>) -> Self {
        let imp = EaxImpl::<Cipher>::with_key_and_nonce(key, nonce);

        Self {
            imp,
            marker: PhantomData,
        }
    }

    /// Process the associated data (AD).
    #[inline]
    pub fn update_assoc(&mut self, aad: &[u8]) {
        self.imp.update_assoc(aad);
    }

    /// Derives the tag from the encrypted/decrypted message so far.
    ///
    /// If the encryption/decryption operation is finished, [`finish`] method
    /// *must* be called instead.
    ///
    ///[`finish`]: #method.finish
    #[inline]
    pub fn tag_clone(&self) -> Tag {
        self.imp.tag_clone()
    }
}

impl<Cipher> Eax<Cipher, Encrypt>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
{
    /// Applies encryption to the plaintext.
    #[inline]
    pub fn encrypt(&mut self, msg: &mut [u8]) {
        self.imp.encrypt(msg)
    }

    /// Finishes the encryption stream, returning the derived tag.
    ///
    /// This *must* be called after the stream encryption is finished.
    #[must_use = "tag must be saved to later verify decrypted data"]
    #[inline]
    pub fn finish(self) -> Tag {
        self.imp.tag()
    }
}

impl<Cipher> Eax<Cipher, Decrypt>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
{
    /// Applies decryption to the ciphertext **without** verifying the
    /// authenticity of decrypted message.
    ///
    /// To correctly verify the authenticity, use the [`finish`] associated
    /// function.
    ///
    /// # ☣️ BEWARE! ☣️
    /// This is a low-level operation that simultaneously decrypts the data and
    /// calculates an intermediate tag used to verify the authenticity of the
    /// data (used when the online decryption is finished).
    ///
    /// Because this is exposed solely as a building block operation, an extra
    /// care must be taken when using this function.
    ///
    /// Specifically, when misused this may be vulnerable to a chosen-ciphertext
    /// attack (IND-CCA). Due to online nature of this function, the decryption
    /// and partial tag calculation is done simultaneously, per chunk.
    /// An attacker might choose ciphertexts to be decrypted and, while the
    /// final decryption will fail because the attacker can't calculate tag
    /// authenticating the message, obtained decryptions may leak information
    /// about the decryption scheme (e.g. leaking parts of the secret key).
    ///
    /// [`finish`]: #method.finish
    #[inline]
    pub fn decrypt_unauthenticated_hazmat(&mut self, msg: &mut [u8]) {
        self.imp.decrypt(msg)
    }

    /// Finishes the decryption stream, verifying whether the associated and
    /// decrypted data stream has not been tampered with.
    ///
    /// This *must* be called after the stream decryption is finished.
    #[must_use = "decrypted data stream must be verified for authenticity"]
    pub fn finish(self, expected: &Tag) -> Result<(), Error> {
        self.imp.verify_ct(expected)
    }
}

/// Implementation of the raw EAX operations.
///
/// Main reason behind extracting the logic to a single, separate type is to
/// facilitate testing of the internal logic.
#[doc(hidden)]
struct EaxImpl<Cipher>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
{
    nonce: Nonce<Cipher::BlockSize>,
    data: Cmac<Cipher>,
    message: Cmac<Cipher>,
    ctr: ctr::Ctr128<Cipher>,
    // HACK: Needed for the test harness due to AEAD trait online/offline interface mismatch
    #[cfg(test)]
    key: Key<Cipher>,
}

impl<Cipher> EaxImpl<Cipher>
where
    Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
    Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
{
    /// Creates a stateful EAX instance that is capable of processing both
    /// the associated data and the plaintext in an "on-line" fashion.
    fn with_key_and_nonce(key: &Key<Cipher>, nonce: &Nonce<Cipher::BlockSize>) -> Self {
        let prepend_cmac = |key, init_val, data| {
            let mut cmac = Cmac::<Cipher>::new(key);
            cmac.update(&[0; 15]);
            cmac.update(&[init_val]);
            cmac.update(data);
            cmac
        };

        // https://crypto.stackexchange.com/questions/26948/eax-cipher-mode-with-nonce-equal-header
        // has an explanation of eax.

        // l = block cipher size = 128 (for AES-128) = 16 byte
        // 1. n ← OMAC(0 || Nonce)
        // (the 0 means the number zero in l bits)
        let n = prepend_cmac(&key, 0, nonce);
        let n = n.finalize().into_bytes();

        // NOTE: These can be updated online later
        // 2. h ← OMAC(1 || associated data)
        let h = prepend_cmac(&key, 1, &[]);
        // 3. c ← OMAC(2 || enc)
        let c = prepend_cmac(&key, 2, &[]);

        let cipher = ctr::Ctr128::<Cipher>::from_block_cipher(Cipher::new(&key), &n);

        Self {
            nonce: n,
            data: h,
            message: c,
            ctr: cipher,
            #[cfg(test)]
            key: key.clone(),
        }
    }

    /// Process the associated data (AD).
    #[inline]
    pub fn update_assoc(&mut self, aad: &[u8]) {
        self.data.update(aad);
    }

    /// Applies encryption to the plaintext.
    #[inline]
    fn encrypt(&mut self, msg: &mut [u8]) {
        self.ctr.apply_keystream(msg);
        self.message.update(msg);
    }

    /// Applies decryption to the ciphertext.
    #[inline]
    fn decrypt(&mut self, msg: &mut [u8]) {
        self.message.update(msg);
        self.ctr.apply_keystream(msg);
    }

    /// Derives the tag from the encrypted/decrypted message so far.
    #[inline]
    fn tag(self) -> Tag {
        let h = self.data.finalize().into_bytes();
        let c = self.message.finalize().into_bytes();

        self.nonce.zip(h, |a, b| a ^ b).zip(c, |a, b| a ^ b)
    }

    /// Derives the tag from the encrypted/decrypted message so far.
    #[inline]
    fn tag_clone(&self) -> Tag {
        let h = self.data.clone().finalize().into_bytes();
        let c = self.message.clone().finalize().into_bytes();

        self.nonce.zip(h, |a, b| a ^ b).zip(c, |a, b| a ^ b)
    }

    /// Finishes the decryption stream, verifying whether the associated and
    /// decrypted data stream has not been tampered with.
    fn verify_ct(self, expected: &Tag) -> Result<(), Error> {
        // Check MAC using secure comparison
        use subtle::ConstantTimeEq;

        let resulting_tag = &self.tag()[..expected.len()];
        if resulting_tag.ct_eq(expected).unwrap_u8() == 1 {
            Ok(())
        } else {
            Err(Error)
        }
    }
}

// Because the current AEAD test harness expects the types to implement both
// `NewAead` and `AeadMutInPlace` traits, do so here so that we can test the
// internal logic used by the public interface for the online EAX variant.
// These are not publicly implemented in general, because the traits are
// designed for offline usage and are somewhat wasteful when used in online mode.
#[cfg(test)]
mod test_impl {
    use super::*;
    use aead::AeadMutInPlace;

    impl<Cipher> NewAead for EaxImpl<Cipher>
    where
        Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
        Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
    {
        type KeySize = Cipher::KeySize;

        fn new(key: &Key<Cipher>) -> Self {
            // HACK: The nonce will be initialized by the appropriate
            // decrypt/encrypt functions from `AeadMutInPlace` implementation.
            // This is currently done so because that trait only implements
            // offline operations and thus need to re-initialize the `EaxImpl`
            // instance.
            let nonce = GenericArray::default();

            Self::with_key_and_nonce(key, &nonce)
        }
    }

    impl<Cipher> AeadMutInPlace for super::EaxImpl<Cipher>
    where
        Cipher: BlockCipher<BlockSize = U16> + NewBlockCipher + Clone,
        Cipher::ParBlocks: ArrayLength<Block<Cipher>>,
    {
        type NonceSize = Cipher::BlockSize;
        type TagSize = <Cmac<Cipher> as Mac>::OutputSize;
        type CiphertextOverhead = U0;

        fn encrypt_in_place_detached(
            &mut self,
            nonce: &Nonce<Self::NonceSize>,
            associated_data: &[u8],
            buffer: &mut [u8],
        ) -> Result<Tag, Error> {
            // HACK: Reinitialize the instance
            *self = Self::with_key_and_nonce(&self.key.clone(), nonce);

            self.update_assoc(associated_data);
            self.encrypt(buffer);

            Ok(self.tag_clone())
        }

        fn decrypt_in_place_detached(
            &mut self,
            nonce: &Nonce<Self::NonceSize>,
            associated_data: &[u8],
            buffer: &mut [u8],
            expected_tag: &Tag,
        ) -> Result<(), Error> {
            // HACK: Reinitialize the instance
            *self = Self::with_key_and_nonce(&self.key.clone(), nonce);

            self.update_assoc(associated_data);
            self.decrypt(buffer);

            let tag = self.tag_clone();

            // Check mac using secure comparison
            use subtle::ConstantTimeEq;
            if expected_tag.ct_eq(&tag).unwrap_u8() == 1 {
                Ok(())
            } else {
                Err(Error)
            }
        }
    }
}

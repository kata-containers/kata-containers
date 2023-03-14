#[cfg(feature = "alloc")]
pub use alloc::vec::Vec;

use crate::errors::{BlockModeError, InvalidKeyIvLength};
use crate::utils::{to_blocks, Block, Key};
use block_padding::Padding;
use cipher::block::{BlockCipher, NewBlockCipher};
use cipher::generic_array::typenum::Unsigned;
use cipher::generic_array::{ArrayLength, GenericArray};

/// Trait for a block cipher mode of operation that is used to apply a block cipher
/// operation to input data to transform it into a variable-length output message.
pub trait BlockMode<C, P>: Sized
where
    C: BlockCipher + NewBlockCipher,
    P: Padding,
{
    /// Initialization Vector size.
    type IvSize: ArrayLength<u8>;

    /// Create a new block mode instance from initialized block cipher and IV.
    fn new(cipher: C, iv: &GenericArray<u8, Self::IvSize>) -> Self;

    /// Create a new block mode instance from fixed sized key and IV.
    fn new_fix(key: &Key<C>, iv: &GenericArray<u8, Self::IvSize>) -> Self {
        Self::new(C::new(key), iv)
    }

    /// Create a new block mode instance from variable size key and IV.
    ///
    /// Returns an error if key or IV have unsupported length.
    fn new_var(key: &[u8], iv: &[u8]) -> Result<Self, InvalidKeyIvLength> {
        if iv.len() != Self::IvSize::USIZE {
            return Err(InvalidKeyIvLength);
        }
        let iv = GenericArray::from_slice(iv);
        let cipher = C::new_varkey(key).map_err(|_| InvalidKeyIvLength)?;
        Ok(Self::new(cipher, iv))
    }

    /// Encrypt blocks of data
    fn encrypt_blocks(&mut self, blocks: &mut [Block<C>]);

    /// Decrypt blocks of data
    fn decrypt_blocks(&mut self, blocks: &mut [Block<C>]);

    /// Encrypt message in-place.
    ///
    /// `&buffer[..pos]` is used as a message and `&buffer[pos..]` as a reserved
    /// space for padding. The padding space should be big enough for padding,
    /// otherwise method will return `Err(BlockModeError)`.
    fn encrypt(mut self, buffer: &mut [u8], pos: usize) -> Result<&[u8], BlockModeError> {
        let bs = C::BlockSize::to_usize();
        let buf = P::pad(buffer, pos, bs).map_err(|_| BlockModeError)?;
        self.encrypt_blocks(to_blocks(buf));
        Ok(buf)
    }

    /// Decrypt message in-place.
    ///
    /// Returns an error if `buffer` length is not multiple of block size and
    /// if after decoding message has malformed padding.
    fn decrypt(mut self, buffer: &mut [u8]) -> Result<&[u8], BlockModeError> {
        let bs = C::BlockSize::to_usize();
        if buffer.len() % bs != 0 {
            return Err(BlockModeError);
        }
        self.decrypt_blocks(to_blocks(buffer));
        P::unpad(buffer).map_err(|_| BlockModeError)
    }

    /// Encrypt message and store result in vector.
    #[cfg(feature = "alloc")]
    fn encrypt_vec(mut self, plaintext: &[u8]) -> Vec<u8> {
        let bs = C::BlockSize::to_usize();
        let pos = plaintext.len();
        let n = pos + bs;
        let mut buf = Vec::with_capacity(n);
        buf.extend_from_slice(plaintext);
        // prepare space for padding
        let block: Block<C> = Default::default();
        buf.extend_from_slice(&block[..n - pos]);

        let n = P::pad(&mut buf, pos, bs)
            .expect("enough space for padding is allocated")
            .len();
        buf.truncate(n);
        self.encrypt_blocks(to_blocks(&mut buf));
        buf
    }

    /// Encrypt message and store result in vector.
    #[cfg(feature = "alloc")]
    fn decrypt_vec(mut self, ciphertext: &[u8]) -> Result<Vec<u8>, BlockModeError> {
        let bs = C::BlockSize::to_usize();
        if ciphertext.len() % bs != 0 {
            return Err(BlockModeError);
        }
        let mut buf = ciphertext.to_vec();
        self.decrypt_blocks(to_blocks(&mut buf));
        let n = P::unpad(&buf).map_err(|_| BlockModeError)?.len();
        buf.truncate(n);
        Ok(buf)
    }
}

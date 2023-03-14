//! Generic implementation of CTR mode with a 32-bit counter
//! (big or little endian), generic over block ciphers.

use cipher::{
    block::{Block, BlockCipher, ParBlocks},
    generic_array::{typenum::Unsigned, ArrayLength, GenericArray},
    stream::{FromBlockCipher, LoopError, SyncStreamCipher},
};
use core::{convert::TryInto, marker::PhantomData, mem};
/// CTR mode with a 32-bit big endian counter.
///
/// Used by e.g. AES-GCM.
pub struct Ctr32BE<B>
where
    B: BlockCipher,
    B::ParBlocks: ArrayLength<GenericArray<u8, B::BlockSize>>,
    Block<B>: Copy,
{
    ctr: Ctr32<B, BigEndian>,
}

/// CTR mode with a 32-bit little endian counter.
///
/// Used by e.g. AES-GCM-SIV.
pub struct Ctr32LE<B>
where
    B: BlockCipher,
    B::ParBlocks: ArrayLength<GenericArray<u8, B::BlockSize>>,
    Block<B>: Copy,
{
    ctr: Ctr32<B, LittleEndian>,
}

impl<B> FromBlockCipher for Ctr32BE<B>
where
    B: BlockCipher,
    B::ParBlocks: ArrayLength<Block<B>>,
    Block<B>: Copy,
{
    type BlockCipher = B;
    type NonceSize = B::BlockSize;

    #[inline]
    fn from_block_cipher(cipher: B, nonce: &Block<B>) -> Self {
        Self {
            ctr: Ctr32::new(cipher, *nonce),
        }
    }
}

impl<B> FromBlockCipher for Ctr32LE<B>
where
    B: BlockCipher,
    B::ParBlocks: ArrayLength<Block<B>>,
    Block<B>: Copy,
{
    type BlockCipher = B;
    type NonceSize = B::BlockSize;

    #[inline]
    fn from_block_cipher(cipher: B, nonce: &Block<B>) -> Self {
        let mut counter_block = *nonce;
        counter_block[15] |= 0x80;

        Self {
            ctr: Ctr32::new(cipher, counter_block),
        }
    }
}

/// Implement stream cipher traits for the given `Ctr32*` type
macro_rules! impl_ctr32 {
    ($ctr32:tt) => {
        impl<B> SyncStreamCipher for $ctr32<B>
        where
            B: BlockCipher,
            B::ParBlocks: ArrayLength<Block<B>>,
            Block<B>: Copy,
        {
            #[inline]
            fn try_apply_keystream(&mut self, data: &mut [u8]) -> Result<(), LoopError> {
                // TODO(tarcieri): data volume limits
                self.ctr.apply_keystream(data);
                Ok(())
            }
        }

        impl<B> $ctr32<B>
        where
            B: BlockCipher,
            B::ParBlocks: ArrayLength<Block<B>>,
            Block<B>: Copy,
        {
            /// Seek to the given NIST SP800-38D counter value.
            ///
            /// Note: the serialized counter value is 1 larger than the argument value.
            // TODO(tarcieri): implement `SyncStreamCipherSeek`
            #[inline]
            pub fn seek_ctr(&mut self, pos: u32) {
                self.ctr.seek(pos);
            }

            /// Get the current NIST SP800-38D counter value.
            // TODO(tarcieri): implement `SyncStreamCipherSeek`
            #[inline]
            pub fn current_ctr(&self) -> u32 {
                self.ctr.current_pos()
            }
        }
    };
}

impl_ctr32!(Ctr32BE);
impl_ctr32!(Ctr32LE);

/// Inner CTR mode implementation with a 32-bit counter, generic over
/// block ciphers and endianness.
struct Ctr32<B, E>
where
    B: BlockCipher,
    B::ParBlocks: ArrayLength<Block<B>>,
    E: Endianness<B>,
    Block<B>: Copy,
{
    /// Cipher
    cipher: B,

    /// Keystream buffer
    buffer: ParBlocks<B>,

    /// Current CTR value
    counter_block: Block<B>,

    /// Base value of the counter
    base_counter: u32,

    /// Endianness
    endianness: PhantomData<E>,
}

impl<B, E> Ctr32<B, E>
where
    B: BlockCipher,
    B::ParBlocks: ArrayLength<GenericArray<u8, B::BlockSize>>,
    E: Endianness<B>,
    Block<B>: Copy,
{
    /// Instantiate a new CTR instance
    pub fn new(cipher: B, counter_block: Block<B>) -> Self {
        Self {
            cipher,
            buffer: unsafe { mem::zeroed() },
            counter_block,
            base_counter: E::get_counter(&counter_block),
            endianness: PhantomData,
        }
    }

    /// "Seek" to the given NIST SP800-38D counter value.
    #[inline]
    pub fn seek(&mut self, new_counter_value: u32) {
        E::set_counter(
            &mut self.counter_block,
            new_counter_value.wrapping_add(self.base_counter),
        );
    }

    /// Get the current NIST SP800-38D counter value.
    #[inline]
    pub fn current_pos(&self) -> u32 {
        E::get_counter(&self.counter_block).wrapping_sub(self.base_counter)
    }

    /// Apply CTR keystream to the given input buffer
    #[inline]
    pub fn apply_keystream(&mut self, msg: &mut [u8]) {
        for chunk in msg.chunks_mut(B::BlockSize::to_usize() * B::ParBlocks::to_usize()) {
            self.apply_keystream_blocks(chunk);
        }
    }

    /// Apply `B::ParBlocks` parallel blocks of keystream to the input buffer
    fn apply_keystream_blocks(&mut self, msg: &mut [u8]) {
        let mut counter = E::get_counter(&self.counter_block);
        let n_blocks = msg.chunks(B::BlockSize::to_usize()).count();
        debug_assert!(n_blocks <= B::ParBlocks::to_usize());

        for block in self.buffer.iter_mut().take(n_blocks) {
            *block = self.counter_block;
            counter = counter.wrapping_add(1);
            E::set_counter(&mut self.counter_block, counter);
        }

        if n_blocks == 1 {
            self.cipher.encrypt_block(&mut self.buffer[0]);
        } else {
            self.cipher.encrypt_blocks(&mut self.buffer);
        }

        for (i, chunk) in msg.chunks_mut(B::BlockSize::to_usize()).enumerate() {
            let keystream_block = &self.buffer[i];

            for (i, byte) in chunk.iter_mut().enumerate() {
                *byte ^= keystream_block[i];
            }
        }
    }
}

/// Endianness-related functionality
trait Endianness<B: BlockCipher> {
    /// Get the counter value from a block
    fn get_counter(block: &Block<B>) -> u32;

    /// Set the counter inside of a block to the given value
    fn set_counter(block: &mut Block<B>, counter: u32);
}

/// Big endian 32-bit counter
struct BigEndian;

impl<B: BlockCipher> Endianness<B> for BigEndian {
    #[inline]
    fn get_counter(block: &Block<B>) -> u32 {
        let offset = B::BlockSize::to_usize() - mem::size_of::<u32>();
        u32::from_be_bytes(block[offset..].try_into().unwrap())
    }

    #[inline]
    fn set_counter(block: &mut Block<B>, value: u32) {
        let offset = B::BlockSize::to_usize() - mem::size_of::<u32>();
        block[offset..].copy_from_slice(&value.to_be_bytes());
    }
}

/// Little endian 32-bit counter
struct LittleEndian;

impl<B: BlockCipher> Endianness<B> for LittleEndian {
    #[inline]
    fn get_counter(block: &Block<B>) -> u32 {
        u32::from_le_bytes(block[..mem::size_of::<u32>()].try_into().unwrap())
    }

    #[inline]
    fn set_counter(block: &mut Block<B>, value: u32) {
        block[..mem::size_of::<u32>()].copy_from_slice(&value.to_le_bytes());
    }
}

pub use cipher::{BlockCipher, NewBlockCipher};

use cipher::{
    consts::{U16, U24, U32, U8},
    generic_array::GenericArray,
};

use crate::{
    fixslice::{self, FixsliceKeys128, FixsliceKeys192, FixsliceKeys256, FIXSLICE_BLOCKS},
    Block, ParBlocks,
};

macro_rules! define_aes_impl {
    (
        $name:ident,
        $key_size:ty,
        $fixslice_keys:ty,
        $fixslice_key_schedule:path,
        $fixslice_decrypt:path,
        $fixslice_encrypt:path,
        $doc:expr
    ) => {
        #[doc=$doc]
        #[derive(Clone)]
        pub struct $name {
            keys: $fixslice_keys,
        }

        impl NewBlockCipher for $name {
            type KeySize = $key_size;

            #[inline]
            fn new(key: &GenericArray<u8, $key_size>) -> Self {
                Self { keys: $fixslice_key_schedule(key) }
            }
        }

        impl BlockCipher for $name {
            type BlockSize = U16;
            type ParBlocks = U8;

            #[inline]
            fn encrypt_block(&self, block: &mut Block) {
                let mut blocks = [Block::default(); FIXSLICE_BLOCKS];
                blocks[0].copy_from_slice(block);
                $fixslice_encrypt(&self.keys, &mut blocks);
                block.copy_from_slice(&blocks[0]);
            }

            #[inline]
            fn decrypt_block(&self, block: &mut Block) {
                let mut blocks = [Block::default(); FIXSLICE_BLOCKS];
                blocks[0].copy_from_slice(block);
                $fixslice_decrypt(&self.keys, &mut blocks);
                block.copy_from_slice(&blocks[0]);
            }

            #[inline]
            fn encrypt_blocks(&self, blocks: &mut ParBlocks) {
                for chunk in blocks.chunks_mut(FIXSLICE_BLOCKS) {
                    $fixslice_encrypt(&self.keys, chunk);
                }
            }

            #[inline]
            fn decrypt_blocks(&self, blocks: &mut ParBlocks) {
                for chunk in blocks.chunks_mut(FIXSLICE_BLOCKS) {
                    $fixslice_decrypt(&self.keys, chunk);
                }
            }
        }

        opaque_debug::implement!($name);
    }
}

define_aes_impl!(
    Aes128,
    U16,
    FixsliceKeys128,
    fixslice::aes128_key_schedule,
    fixslice::aes128_decrypt,
    fixslice::aes128_encrypt,
    "AES-128 block cipher instance"
);

define_aes_impl!(
    Aes192,
    U24,
    FixsliceKeys192,
    fixslice::aes192_key_schedule,
    fixslice::aes192_decrypt,
    fixslice::aes192_encrypt,
    "AES-192 block cipher instance"
);

define_aes_impl!(
    Aes256,
    U32,
    FixsliceKeys256,
    fixslice::aes256_key_schedule,
    fixslice::aes256_decrypt,
    fixslice::aes256_encrypt,
    "AES-256 block cipher instance"
);

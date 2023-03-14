use crate::traits::BlockMode;
use crate::utils::{xor, Block};
use block_padding::Padding;
use cipher::block::{BlockCipher, NewBlockCipher};
use cipher::generic_array::GenericArray;
use core::marker::PhantomData;

/// [Output feedback][1] (OFB) block mode instance with a full block feedback.
///
/// [1]: https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation#Cipher_feedback_(CFB)
#[derive(Clone)]
pub struct Ofb<C: BlockCipher + NewBlockCipher, P: Padding> {
    cipher: C,
    iv: GenericArray<u8, C::BlockSize>,
    _p: PhantomData<P>,
}

impl<C, P> BlockMode<C, P> for Ofb<C, P>
where
    C: BlockCipher + NewBlockCipher,
    P: Padding,
{
    type IvSize = C::BlockSize;

    fn new(cipher: C, iv: &Block<C>) -> Self {
        Self {
            cipher,
            iv: iv.clone(),
            _p: Default::default(),
        }
    }

    fn encrypt_blocks(&mut self, blocks: &mut [Block<C>]) {
        for block in blocks.iter_mut() {
            self.cipher.encrypt_block(&mut self.iv);
            xor(block, &self.iv);
        }
    }

    fn decrypt_blocks(&mut self, blocks: &mut [Block<C>]) {
        self.encrypt_blocks(blocks)
    }
}

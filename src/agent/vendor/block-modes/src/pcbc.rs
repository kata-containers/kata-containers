use crate::traits::BlockMode;
use crate::utils::{xor, Block};
use block_padding::Padding;
use cipher::block::{BlockCipher, NewBlockCipher};
use cipher::generic_array::GenericArray;
use core::marker::PhantomData;

/// [Propagating Cipher Block Chaining][1] (PCBC) mode instance.
///
/// [1]: https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation#PCBC
#[derive(Clone)]
pub struct Pcbc<C: BlockCipher + NewBlockCipher, P: Padding> {
    cipher: C,
    iv: GenericArray<u8, C::BlockSize>,
    _p: PhantomData<P>,
}

impl<C, P> Pcbc<C, P>
where
    C: BlockCipher + NewBlockCipher,
    P: Padding,
{
    /// Initialize PCBC
    pub fn new(cipher: C, iv: &Block<C>) -> Self {
        Self {
            cipher,
            iv: iv.clone(),
            _p: Default::default(),
        }
    }
}

impl<C, P> BlockMode<C, P> for Pcbc<C, P>
where
    C: BlockCipher + NewBlockCipher,
    P: Padding,
{
    type IvSize = C::BlockSize;

    fn new(cipher: C, iv: &GenericArray<u8, C::BlockSize>) -> Self {
        Self {
            cipher,
            iv: iv.clone(),
            _p: Default::default(),
        }
    }

    fn encrypt_blocks(&mut self, blocks: &mut [Block<C>]) {
        for block in blocks {
            let plaintext = block.clone();
            xor(block, &self.iv);
            self.cipher.encrypt_block(block);
            self.iv = plaintext;
            xor(&mut self.iv, block);
        }
    }

    fn decrypt_blocks(&mut self, blocks: &mut [Block<C>]) {
        for block in blocks {
            let ciphertext = block.clone();
            self.cipher.decrypt_block(block);
            xor(block, &self.iv);
            self.iv = ciphertext;
            xor(&mut self.iv, block);
        }
    }
}

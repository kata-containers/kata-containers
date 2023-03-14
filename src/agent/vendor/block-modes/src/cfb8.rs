use crate::traits::BlockMode;
use crate::utils::Block;
use block_padding::Padding;
use cipher::block::{BlockCipher, NewBlockCipher};
use cipher::generic_array::GenericArray;
use core::marker::PhantomData;

/// [Cipher feedback][1] (CFB) block mode instance with a full block feedback.
///
/// [1]: https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation#Cipher_feedback_(CFB)
#[derive(Clone)]
pub struct Cfb8<C: BlockCipher + NewBlockCipher, P: Padding> {
    cipher: C,
    iv: GenericArray<u8, C::BlockSize>,
    _p: PhantomData<P>,
}

impl<C, P> BlockMode<C, P> for Cfb8<C, P>
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
        let mut iv = self.iv.clone();
        let n = iv.len();
        for block in blocks.iter_mut() {
            for b in block.iter_mut() {
                let iv_copy = iv.clone();
                self.cipher.encrypt_block(&mut iv);
                *b ^= iv[0];
                iv[..n - 1].clone_from_slice(&iv_copy[1..]);
                iv[n - 1] = *b;
            }
        }
        self.iv = iv;
    }

    fn decrypt_blocks(&mut self, blocks: &mut [Block<C>]) {
        let mut iv = self.iv.clone();
        let n = iv.len();
        for block in blocks.iter_mut() {
            for b in block.iter_mut() {
                let iv_copy = iv.clone();
                self.cipher.encrypt_block(&mut iv);
                let t = *b;
                *b ^= iv[0];
                iv[..n - 1].clone_from_slice(&iv_copy[1..]);
                iv[n - 1] = t;
            }
        }
        self.iv = iv;
    }
}

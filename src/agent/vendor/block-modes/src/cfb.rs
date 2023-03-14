use crate::traits::BlockMode;
use crate::utils::{xor, Block, ParBlocks};
use block_padding::Padding;
use cipher::block::{BlockCipher, NewBlockCipher};
use cipher::generic_array::typenum::Unsigned;
use cipher::generic_array::GenericArray;
use core::marker::PhantomData;
use core::ptr;

/// [Cipher feedback][1] (CFB) block mode instance with a full block feedback.
///
/// [1]: https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation#Cipher_feedback_(CFB)
#[derive(Clone)]
pub struct Cfb<C: BlockCipher + BlockCipher, P: Padding> {
    cipher: C,
    iv: GenericArray<u8, C::BlockSize>,
    _p: PhantomData<P>,
}

impl<C, P> BlockMode<C, P> for Cfb<C, P>
where
    C: BlockCipher + NewBlockCipher,
    P: Padding,
{
    type IvSize = C::BlockSize;

    fn new(cipher: C, iv: &Block<C>) -> Self {
        let mut iv = iv.clone();
        cipher.encrypt_block(&mut iv);
        Self {
            cipher,
            iv,
            _p: Default::default(),
        }
    }

    fn encrypt_blocks(&mut self, blocks: &mut [Block<C>]) {
        for block in blocks {
            xor_set1(block, self.iv.as_mut_slice());
            self.cipher.encrypt_block(&mut self.iv);
        }
    }

    fn decrypt_blocks(&mut self, mut blocks: &mut [Block<C>]) {
        let pb = C::ParBlocks::to_usize();

        if blocks.len() >= pb {
            // SAFETY: we have checked that `blocks` has enough elements
            #[allow(unsafe_code)]
            let mut par_iv = read_par_block::<C>(blocks);
            self.cipher.encrypt_blocks(&mut par_iv);

            let (b, r) = { blocks }.split_at_mut(1);
            blocks = r;

            xor(&mut b[0], &self.iv);

            while blocks.len() >= 2 * pb - 1 {
                let next_par_iv = read_par_block::<C>(&blocks[pb - 1..]);

                let (par_block, r) = { blocks }.split_at_mut(pb);
                blocks = r;

                for (a, b) in par_block.iter_mut().zip(par_iv.iter()) {
                    xor(a, b)
                }
                par_iv = next_par_iv;
                self.cipher.encrypt_blocks(&mut par_iv);
            }

            let (par_block, r) = { blocks }.split_at_mut(pb - 1);
            blocks = r;

            for (a, b) in par_block.iter_mut().zip(par_iv[..pb - 1].iter()) {
                xor(a, b)
            }
            self.iv = par_iv[pb - 1].clone();
        }

        for block in blocks {
            xor_set2(block, self.iv.as_mut_slice());
            self.cipher.encrypt_block(&mut self.iv);
        }
    }
}

#[inline(always)]
fn read_par_block<C: BlockCipher>(blocks: &[Block<C>]) -> ParBlocks<C> {
    assert!(blocks.len() >= C::ParBlocks::to_usize());
    // SAFETY: assert checks that `blocks` is long enough
    #[allow(unsafe_code)]
    unsafe {
        ptr::read(blocks.as_ptr() as *const ParBlocks<C>)
    }
}

#[inline(always)]
fn xor_set1(buf1: &mut [u8], buf2: &mut [u8]) {
    for (a, b) in buf1.iter_mut().zip(buf2) {
        let t = *a ^ *b;
        *a = t;
        *b = t;
    }
}

#[inline(always)]
fn xor_set2(buf1: &mut [u8], buf2: &mut [u8]) {
    for (a, b) in buf1.iter_mut().zip(buf2) {
        let t = *a;
        *a ^= *b;
        *b = t;
    }
}

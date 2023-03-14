use crate::errors::InvalidKeyIvLength;
use crate::traits::BlockMode;
use crate::utils::{get_par_blocks, Block};
use block_padding::Padding;
use cipher::block::{BlockCipher, NewBlockCipher};
use cipher::generic_array::typenum::{Unsigned, U0};
use cipher::generic_array::GenericArray;
use core::marker::PhantomData;

/// [Electronic Codebook][1] (ECB) block cipher mode instance.
///
/// Note that `new` method ignores IV, so during initialization you can
/// just pass `Default::default()` instead.
///
/// [1]: https://en.wikipedia.org/wiki/Block_cipher_mode_of_operation#ECB
#[derive(Clone)]
pub struct Ecb<C: BlockCipher + BlockCipher, P: Padding> {
    cipher: C,
    _p: PhantomData<P>,
}

impl<C, P> BlockMode<C, P> for Ecb<C, P>
where
    C: BlockCipher + NewBlockCipher,
    P: Padding,
{
    type IvSize = U0;

    fn new(cipher: C, _iv: &GenericArray<u8, U0>) -> Self {
        Self {
            cipher,
            _p: Default::default(),
        }
    }

    fn new_var(key: &[u8], _iv: &[u8]) -> Result<Self, InvalidKeyIvLength> {
        let cipher = C::new_varkey(key).map_err(|_| InvalidKeyIvLength)?;
        Ok(Self {
            cipher,
            _p: Default::default(),
        })
    }

    fn encrypt_blocks(&mut self, blocks: &mut [Block<C>]) {
        if C::ParBlocks::to_usize() != 1 {
            let (par_blocks, blocks) = get_par_blocks::<C>(blocks);
            par_blocks
                .iter_mut()
                .for_each(|pb| self.cipher.encrypt_blocks(pb));
            blocks
                .iter_mut()
                .for_each(|pb| self.cipher.encrypt_block(pb));
        } else {
            blocks
                .iter_mut()
                .for_each(|pb| self.cipher.encrypt_block(pb));
        }
    }

    fn decrypt_blocks(&mut self, blocks: &mut [Block<C>]) {
        if C::ParBlocks::to_usize() != 1 {
            let (par_blocks, blocks) = get_par_blocks::<C>(blocks);
            par_blocks
                .iter_mut()
                .for_each(|pb| self.cipher.decrypt_blocks(pb));
            blocks
                .iter_mut()
                .for_each(|pb| self.cipher.decrypt_block(pb));
        } else {
            blocks
                .iter_mut()
                .for_each(|pb| self.cipher.decrypt_block(pb));
        }
    }
}

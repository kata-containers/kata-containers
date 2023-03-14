use cipher::block::{BlockCipher, NewBlockCipher};
use cipher::generic_array::typenum::Unsigned;
use cipher::generic_array::{ArrayLength, GenericArray};
use core::slice;

#[inline(always)]
pub fn xor(buf: &mut [u8], key: &[u8]) {
    debug_assert_eq!(buf.len(), key.len());
    for (a, b) in buf.iter_mut().zip(key) {
        *a ^= *b;
    }
}

pub(crate) type Key<C> = GenericArray<u8, <C as NewBlockCipher>::KeySize>;
pub(crate) type Block<C> = GenericArray<u8, <C as BlockCipher>::BlockSize>;
pub(crate) type ParBlocks<C> = GenericArray<Block<C>, <C as BlockCipher>::ParBlocks>;

pub(crate) fn to_blocks<N>(data: &mut [u8]) -> &mut [GenericArray<u8, N>]
where
    N: ArrayLength<u8>,
{
    let n = N::to_usize();
    debug_assert!(data.len() % n == 0);

    #[allow(unsafe_code)]
    unsafe {
        slice::from_raw_parts_mut(data.as_ptr() as *mut GenericArray<u8, N>, data.len() / n)
    }
}

pub(crate) fn get_par_blocks<C: BlockCipher>(
    blocks: &mut [Block<C>],
) -> (&mut [ParBlocks<C>], &mut [Block<C>]) {
    let pb = C::ParBlocks::to_usize();
    let n_par = blocks.len() / pb;

    let (par, single) = blocks.split_at_mut(n_par * pb);

    #[allow(unsafe_code)]
    let par = unsafe { slice::from_raw_parts_mut(par.as_ptr() as *mut ParBlocks<C>, n_par) };
    (par, single)
}

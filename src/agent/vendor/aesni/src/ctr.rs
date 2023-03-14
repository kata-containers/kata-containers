#![allow(clippy::unreadable_literal)]

use crate::arch::*;
use core::mem;

use super::{Aes128, Aes192, Aes256};
use cipher::stream::{
    FromBlockCipher, LoopError, OverflowError, SeekNum, SyncStreamCipher, SyncStreamCipherSeek,
};
use cipher::{consts::U16, generic_array::GenericArray, BlockCipher};

const BLOCK_SIZE: usize = 16;
const PAR_BLOCKS: usize = 8;
const PAR_BLOCKS_SIZE: usize = PAR_BLOCKS * BLOCK_SIZE;

#[inline(always)]
pub fn xor(buf: &mut [u8], key: &[u8]) {
    debug_assert_eq!(buf.len(), key.len());
    for (a, b) in buf.iter_mut().zip(key) {
        *a ^= *b;
    }
}

#[inline(always)]
fn xor_block8(buf: &mut [u8], ctr: [__m128i; 8]) {
    debug_assert_eq!(buf.len(), PAR_BLOCKS_SIZE);

    // Safety: `loadu` and `storeu` support unaligned access
    #[allow(clippy::cast_ptr_alignment)]
    unsafe {
        // compiler should unroll this loop
        for i in 0..8 {
            let ptr = buf.as_mut_ptr().offset(16 * i) as *mut __m128i;
            let data = _mm_loadu_si128(ptr);
            let data = _mm_xor_si128(data, ctr[i as usize]);
            _mm_storeu_si128(ptr, data);
        }
    }
}

#[inline(always)]
fn swap_bytes(v: __m128i) -> __m128i {
    unsafe {
        let mask = _mm_set_epi64x(0x08090a0b0c0d0e0f, 0x0001020304050607);
        _mm_shuffle_epi8(v, mask)
    }
}

#[inline(always)]
fn inc_be(v: __m128i) -> __m128i {
    unsafe { _mm_add_epi64(v, _mm_set_epi64x(1, 0)) }
}

#[inline(always)]
fn load(val: &GenericArray<u8, U16>) -> __m128i {
    // Safety: `loadu` supports unaligned loads
    #[allow(clippy::cast_ptr_alignment)]
    unsafe {
        _mm_loadu_si128(val.as_ptr() as *const __m128i)
    }
}

macro_rules! impl_ctr {
    ($name:ident, $cipher:ty, $doc:expr) => {
        #[doc=$doc]
        #[derive(Clone)]
        pub struct $name {
            nonce: __m128i,
            ctr: __m128i,
            cipher: $cipher,
            block: [u8; BLOCK_SIZE],
            pos: u8,
        }

        impl $name {
            #[inline(always)]
            fn gen_block(&mut self) {
                let block = self.cipher.encrypt(swap_bytes(self.ctr));
                self.block = unsafe { mem::transmute(block) }
            }

            #[inline(always)]
            fn next_block(&mut self) -> __m128i {
                let block = swap_bytes(self.ctr);
                self.ctr = inc_be(self.ctr);
                self.cipher.encrypt(block)
            }

            #[inline(always)]
            fn next_block8(&mut self) -> [__m128i; 8] {
                let mut ctr = self.ctr;
                let mut block8: [__m128i; 8] = unsafe { mem::zeroed() };
                for i in 0..8 {
                    block8[i] = swap_bytes(ctr);
                    ctr = inc_be(ctr);
                }
                self.ctr = ctr;

                self.cipher.encrypt8(block8)
            }

            #[inline(always)]
            fn get_u64_ctr(&self) -> u64 {
                let (ctr, nonce) = unsafe {(
                    mem::transmute::<__m128i, [u64; 2]>(self.ctr)[1],
                    mem::transmute::<__m128i, [u64; 2]>(self.nonce)[1],
                )};
                ctr.wrapping_sub(nonce)
            }

            /// Check if provided data will not overflow counter
            #[inline(always)]
            fn check_data_len(&self, data: &[u8]) -> Result<(), LoopError> {
                let bs = BLOCK_SIZE;
                let leftover_bytes = bs - self.pos as usize;
                if data.len() < leftover_bytes {
                    return Ok(());
                }
                let blocks = 1 + (data.len() - leftover_bytes) / bs;
                self.get_u64_ctr()
                    .checked_add(blocks as u64)
                    .ok_or(LoopError)
                    .map(|_| ())
            }
        }

        impl FromBlockCipher for $name {
            type BlockCipher = $cipher;
            type NonceSize = <$cipher as BlockCipher>::BlockSize;

            fn from_block_cipher(
                cipher: $cipher,
                nonce: &GenericArray<u8, Self::NonceSize>,
            ) -> Self {
                let nonce = swap_bytes(load(nonce));
                Self {
                    nonce,
                    ctr: nonce,
                    cipher,
                    block: [0u8; BLOCK_SIZE],
                    pos: 0,
                }
            }
        }

        impl SyncStreamCipher for $name {
            #[inline]
            fn try_apply_keystream(&mut self, mut data: &mut [u8])
                -> Result<(), LoopError>
            {
                self.check_data_len(data)?;
                let bs = BLOCK_SIZE;
                let pos = self.pos as usize;
                debug_assert!(bs > pos);

                if pos != 0 {
                    if data.len() < bs - pos {
                        let n = pos + data.len();
                        xor(data, &self.block[pos..n]);
                        self.pos = n as u8;
                        return Ok(());
                    } else {
                        let (l, r) = data.split_at_mut(bs - pos);
                        data = r;
                        xor(l, &self.block[pos..]);
                        self.ctr = inc_be(self.ctr);
                    }
                }

                let mut chunks = data.chunks_exact_mut(PAR_BLOCKS_SIZE);
                for chunk in &mut chunks {
                    xor_block8(chunk, self.next_block8());
                }
                data = chunks.into_remainder();

                let mut chunks = data.chunks_exact_mut(bs);
                for chunk in &mut chunks {
                    let block = self.next_block();

                    unsafe {
                        let t = _mm_loadu_si128(chunk.as_ptr() as *const __m128i);
                        let res = _mm_xor_si128(block, t);
                        _mm_storeu_si128(chunk.as_mut_ptr() as *mut __m128i, res);
                    }
                }

                let rem = chunks.into_remainder();
                self.pos = rem.len() as u8;
                if !rem.is_empty() {
                    self.gen_block();
                    for (a, b) in rem.iter_mut().zip(&self.block) {
                        *a ^= *b;
                    }
                }

                Ok(())
            }
        }

        impl SyncStreamCipherSeek for $name {
            fn try_current_pos<T: SeekNum>(&self) -> Result<T, OverflowError> {
                T::from_block_byte(self.get_u64_ctr(), self.pos, BLOCK_SIZE as u8)
            }

            fn try_seek<T: SeekNum>(&mut self, pos: T) -> Result<(), LoopError> {
                let res: (u64, u8) = pos.to_block_byte(BLOCK_SIZE as u8)?;
                self.ctr = unsafe {
                    _mm_add_epi64(self.nonce, _mm_set_epi64x(res.0 as i64, 0))
                };
                self.pos = res.1;
                if self.pos != 0 {
                    self.gen_block()
                }
                Ok(())
            }
        }

        opaque_debug::implement!($name);
    }
}

impl_ctr!(Aes128Ctr, Aes128, "AES-128 in CTR mode");
impl_ctr!(Aes192Ctr, Aes192, "AES-192 in CTR mode");
impl_ctr!(Aes256Ctr, Aes256, "AES-256 in CTR mode");

use crate::arch::*;

use core::mem;

macro_rules! expand_round {
    ($enc_keys:expr, $dec_keys:expr, $pos:expr, $round:expr) => {
        let mut t1 = _mm_load_si128($enc_keys.as_ptr().offset($pos - 1));
        let mut t2;
        let mut t3;

        t2 = _mm_aeskeygenassist_si128(t1, $round);
        t2 = _mm_shuffle_epi32(t2, 0xff);
        t3 = _mm_slli_si128(t1, 0x4);
        t1 = _mm_xor_si128(t1, t3);
        t3 = _mm_slli_si128(t3, 0x4);
        t1 = _mm_xor_si128(t1, t3);
        t3 = _mm_slli_si128(t3, 0x4);
        t1 = _mm_xor_si128(t1, t3);
        t1 = _mm_xor_si128(t1, t2);

        _mm_store_si128($enc_keys.as_mut_ptr().offset($pos), t1);
        let t1 = if $pos != 10 { _mm_aesimc_si128(t1) } else { t1 };
        _mm_store_si128($dec_keys.as_mut_ptr().offset($pos), t1);
    };
}

#[inline(always)]
pub(super) fn expand(key: &[u8; 16]) -> ([__m128i; 11], [__m128i; 11]) {
    unsafe {
        let mut enc_keys: [__m128i; 11] = mem::zeroed();
        let mut dec_keys: [__m128i; 11] = mem::zeroed();

        // Safety: `loadu` supports unaligned loads
        #[allow(clippy::cast_ptr_alignment)]
        let k = _mm_loadu_si128(key.as_ptr() as *const __m128i);
        _mm_store_si128(enc_keys.as_mut_ptr(), k);
        _mm_store_si128(dec_keys.as_mut_ptr(), k);

        expand_round!(enc_keys, dec_keys, 1, 0x01);
        expand_round!(enc_keys, dec_keys, 2, 0x02);
        expand_round!(enc_keys, dec_keys, 3, 0x04);
        expand_round!(enc_keys, dec_keys, 4, 0x08);
        expand_round!(enc_keys, dec_keys, 5, 0x10);
        expand_round!(enc_keys, dec_keys, 6, 0x20);
        expand_round!(enc_keys, dec_keys, 7, 0x40);
        expand_round!(enc_keys, dec_keys, 8, 0x80);
        expand_round!(enc_keys, dec_keys, 9, 0x1B);
        expand_round!(enc_keys, dec_keys, 10, 0x36);

        (enc_keys, dec_keys)
    }
}

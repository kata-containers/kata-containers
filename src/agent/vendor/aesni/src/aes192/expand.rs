use crate::arch::*;

use core::{mem, ptr};

macro_rules! expand_round {
    ($t1:expr, $t3:expr, $round:expr) => {{
        let mut t1 = $t1;
        let mut t2;
        let mut t3 = $t3;
        let mut t4;

        t2 = _mm_aeskeygenassist_si128(t3, $round);
        t2 = _mm_shuffle_epi32(t2, 0x55);
        t4 = _mm_slli_si128(t1, 0x4);
        t1 = _mm_xor_si128(t1, t4);
        t4 = _mm_slli_si128(t4, 0x4);
        t1 = _mm_xor_si128(t1, t4);
        t4 = _mm_slli_si128(t4, 0x4);
        t1 = _mm_xor_si128(t1, t4);
        t1 = _mm_xor_si128(t1, t2);
        t2 = _mm_shuffle_epi32(t1, 0xff);
        t4 = _mm_slli_si128(t3, 0x4);
        t3 = _mm_xor_si128(t3, t4);
        t3 = _mm_xor_si128(t3, t2);

        (t1, t3)
    }};
}

macro_rules! shuffle {
    ($a:expr, $b:expr, $imm:expr) => {
        mem::transmute::<_, __m128i>(_mm_shuffle_pd(mem::transmute($a), mem::transmute($b), $imm))
    };
}

#[inline(always)]
pub(super) fn expand(key: &[u8; 24]) -> ([__m128i; 13], [__m128i; 13]) {
    unsafe {
        let mut enc_keys: [__m128i; 13] = mem::zeroed();
        let mut dec_keys: [__m128i; 13] = mem::zeroed();

        macro_rules! store {
            ($i:expr, $k:expr) => {
                _mm_store_si128(enc_keys.as_mut_ptr().offset($i), $k);
                _mm_store_si128(dec_keys.as_mut_ptr().offset($i), _mm_aesimc_si128($k));
            };
        }

        // we are being extra pedantic here to remove out-of-bound access.
        // this should be optimized out into movups, movsd sequence
        // note that unaligned load MUST be used here, even though we read
        // from the array (compiler missoptimizes aligned load)
        let (k0, k1l) = {
            let mut t = [0u8; 32];
            ptr::write(t.as_mut_ptr() as *mut [u8; 24], *key);

            // Safety: `loadu` supports unaligned loads
            #[allow(clippy::cast_ptr_alignment)]
            (
                _mm_loadu_si128(t.as_ptr() as *const __m128i),
                _mm_loadu_si128(t.as_ptr().offset(16) as *const __m128i),
            )
        };

        _mm_store_si128(enc_keys.as_mut_ptr(), k0);
        _mm_store_si128(dec_keys.as_mut_ptr(), k0);

        let (k1_2, k2r) = expand_round!(k0, k1l, 0x01);
        let k1 = shuffle!(k1l, k1_2, 0);
        let k2 = shuffle!(k1_2, k2r, 1);
        store!(1, k1);
        store!(2, k2);

        let (k3, k4l) = expand_round!(k1_2, k2r, 0x02);
        store!(3, k3);

        let (k4_5, k5r) = expand_round!(k3, k4l, 0x04);
        let k4 = shuffle!(k4l, k4_5, 0);
        let k5 = shuffle!(k4_5, k5r, 1);
        store!(4, k4);
        store!(5, k5);

        let (k6, k7l) = expand_round!(k4_5, k5r, 0x08);
        store!(6, k6);

        let (k7_8, k8r) = expand_round!(k6, k7l, 0x10);
        let k7 = shuffle!(k7l, k7_8, 0);
        let k8 = shuffle!(k7_8, k8r, 1);
        store!(7, k7);
        store!(8, k8);

        let (k9, k10l) = expand_round!(k7_8, k8r, 0x20);
        store!(9, k9);

        let (k10_11, k11r) = expand_round!(k9, k10l, 0x40);
        let k10 = shuffle!(k10l, k10_11, 0);
        let k11 = shuffle!(k10_11, k11r, 1);
        store!(10, k10);
        store!(11, k11);

        let (k12, _) = expand_round!(k10_11, k11r, 0x80);
        _mm_store_si128(enc_keys.as_mut_ptr().offset(12), k12);
        _mm_store_si128(dec_keys.as_mut_ptr().offset(12), k12);

        (enc_keys, dec_keys)
    }
}

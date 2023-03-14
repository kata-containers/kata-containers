#[cfg(test)]
use crate::arch::__m128i;
#[cfg(test)]
use core::mem;

use cipher::{
    consts::{U16, U8},
    generic_array::GenericArray,
};

pub type Block128 = GenericArray<u8, U16>;
pub type Block128x8 = GenericArray<GenericArray<u8, U16>, U8>;

#[cfg(test)]
pub(crate) fn check(a: &[__m128i], b: &[[u64; 2]]) {
    for (v1, v2) in a.iter().zip(b) {
        let t1: [u64; 2] = unsafe { mem::transmute(*v1) };
        let t2 = [v2[0].to_be(), v2[1].to_be()];
        assert_eq!(t1, t2);
    }
}

macro_rules! load8 {
    ($blocks:expr) => {
        [
            _mm_loadu_si128($blocks[0].as_ptr() as *const __m128i),
            _mm_loadu_si128($blocks[1].as_ptr() as *const __m128i),
            _mm_loadu_si128($blocks[2].as_ptr() as *const __m128i),
            _mm_loadu_si128($blocks[3].as_ptr() as *const __m128i),
            _mm_loadu_si128($blocks[4].as_ptr() as *const __m128i),
            _mm_loadu_si128($blocks[5].as_ptr() as *const __m128i),
            _mm_loadu_si128($blocks[6].as_ptr() as *const __m128i),
            _mm_loadu_si128($blocks[7].as_ptr() as *const __m128i),
        ]
    };
}

macro_rules! store8 {
    ($blocks:expr, $b:expr) => {
        _mm_storeu_si128($blocks[0].as_mut_ptr() as *mut __m128i, $b[0]);
        _mm_storeu_si128($blocks[1].as_mut_ptr() as *mut __m128i, $b[1]);
        _mm_storeu_si128($blocks[2].as_mut_ptr() as *mut __m128i, $b[2]);
        _mm_storeu_si128($blocks[3].as_mut_ptr() as *mut __m128i, $b[3]);
        _mm_storeu_si128($blocks[4].as_mut_ptr() as *mut __m128i, $b[4]);
        _mm_storeu_si128($blocks[5].as_mut_ptr() as *mut __m128i, $b[5]);
        _mm_storeu_si128($blocks[6].as_mut_ptr() as *mut __m128i, $b[6]);
        _mm_storeu_si128($blocks[7].as_mut_ptr() as *mut __m128i, $b[7]);
    };
}

macro_rules! xor8 {
    ($b:expr, $key:expr) => {
        $b[0] = _mm_xor_si128($b[0], $key);
        $b[1] = _mm_xor_si128($b[1], $key);
        $b[2] = _mm_xor_si128($b[2], $key);
        $b[3] = _mm_xor_si128($b[3], $key);
        $b[4] = _mm_xor_si128($b[4], $key);
        $b[5] = _mm_xor_si128($b[5], $key);
        $b[6] = _mm_xor_si128($b[6], $key);
        $b[7] = _mm_xor_si128($b[7], $key);
    };
}

macro_rules! aesenc8 {
    ($b:expr, $key:expr) => {
        $b[0] = _mm_aesenc_si128($b[0], $key);
        $b[1] = _mm_aesenc_si128($b[1], $key);
        $b[2] = _mm_aesenc_si128($b[2], $key);
        $b[3] = _mm_aesenc_si128($b[3], $key);
        $b[4] = _mm_aesenc_si128($b[4], $key);
        $b[5] = _mm_aesenc_si128($b[5], $key);
        $b[6] = _mm_aesenc_si128($b[6], $key);
        $b[7] = _mm_aesenc_si128($b[7], $key);
    };
}

macro_rules! aesenclast8 {
    ($b:expr, $key:expr) => {
        $b[0] = _mm_aesenclast_si128($b[0], $key);
        $b[1] = _mm_aesenclast_si128($b[1], $key);
        $b[2] = _mm_aesenclast_si128($b[2], $key);
        $b[3] = _mm_aesenclast_si128($b[3], $key);
        $b[4] = _mm_aesenclast_si128($b[4], $key);
        $b[5] = _mm_aesenclast_si128($b[5], $key);
        $b[6] = _mm_aesenclast_si128($b[6], $key);
        $b[7] = _mm_aesenclast_si128($b[7], $key);
    };
}

macro_rules! aesdec8 {
    ($b:expr, $key:expr) => {
        $b[0] = _mm_aesdec_si128($b[0], $key);
        $b[1] = _mm_aesdec_si128($b[1], $key);
        $b[2] = _mm_aesdec_si128($b[2], $key);
        $b[3] = _mm_aesdec_si128($b[3], $key);
        $b[4] = _mm_aesdec_si128($b[4], $key);
        $b[5] = _mm_aesdec_si128($b[5], $key);
        $b[6] = _mm_aesdec_si128($b[6], $key);
        $b[7] = _mm_aesdec_si128($b[7], $key);
    };
}

macro_rules! aesdeclast8 {
    ($b:expr, $key:expr) => {
        $b[0] = _mm_aesdeclast_si128($b[0], $key);
        $b[1] = _mm_aesdeclast_si128($b[1], $key);
        $b[2] = _mm_aesdeclast_si128($b[2], $key);
        $b[3] = _mm_aesdeclast_si128($b[3], $key);
        $b[4] = _mm_aesdeclast_si128($b[4], $key);
        $b[5] = _mm_aesdeclast_si128($b[5], $key);
        $b[6] = _mm_aesdeclast_si128($b[6], $key);
        $b[7] = _mm_aesdeclast_si128($b[7], $key);
    };
}

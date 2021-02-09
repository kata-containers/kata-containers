#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

use crate::guts::{
    assemble_count, count_high, count_low, final_block, flag_word, input_debug_asserts, Finalize,
    Job, Stride,
};
use crate::{Word, BLOCKBYTES, IV, SIGMA};
use arrayref::{array_refs, mut_array_refs};
use core::cmp;
use core::mem;

pub const DEGREE: usize = 2;

#[inline(always)]
unsafe fn loadu(src: *const [Word; DEGREE]) -> __m128i {
    // This is an unaligned load, so the pointer cast is allowed.
    _mm_loadu_si128(src as *const __m128i)
}

#[inline(always)]
unsafe fn storeu(src: __m128i, dest: *mut [Word; DEGREE]) {
    // This is an unaligned store, so the pointer cast is allowed.
    _mm_storeu_si128(dest as *mut __m128i, src)
}

#[inline(always)]
unsafe fn add(a: __m128i, b: __m128i) -> __m128i {
    _mm_add_epi64(a, b)
}

#[inline(always)]
unsafe fn eq(a: __m128i, b: __m128i) -> __m128i {
    _mm_cmpeq_epi64(a, b)
}

#[inline(always)]
unsafe fn and(a: __m128i, b: __m128i) -> __m128i {
    _mm_and_si128(a, b)
}

#[inline(always)]
unsafe fn negate_and(a: __m128i, b: __m128i) -> __m128i {
    // Note that "and not" implies the reverse of the actual arg order.
    _mm_andnot_si128(a, b)
}

#[inline(always)]
unsafe fn xor(a: __m128i, b: __m128i) -> __m128i {
    _mm_xor_si128(a, b)
}

#[inline(always)]
unsafe fn set1(x: u64) -> __m128i {
    _mm_set1_epi64x(x as i64)
}

#[inline(always)]
unsafe fn set2(a: u64, b: u64) -> __m128i {
    // There's no _mm_setr_epi64x, so note the arg order is backwards.
    _mm_set_epi64x(b as i64, a as i64)
}

// Adapted from https://github.com/rust-lang-nursery/stdsimd/pull/479.
macro_rules! _MM_SHUFFLE {
    ($z:expr, $y:expr, $x:expr, $w:expr) => {
        ($z << 6) | ($y << 4) | ($x << 2) | $w
    };
}

// These rotations are the "simple version". For the "complicated version", see
// https://github.com/sneves/blake2-avx2/blob/b3723921f668df09ece52dcd225a36d4a4eea1d9/blake2b-common.h#L43-L46.
// For a discussion of the tradeoffs, see
// https://github.com/sneves/blake2-avx2/pull/5. In short:
// - Due to an LLVM bug (https://bugs.llvm.org/show_bug.cgi?id=44379), this
//   version performs better on recent x86 chips.
// - LLVM is able to optimize this version to AVX-512 rotation instructions
//   when those are enabled.

#[inline(always)]
unsafe fn rot32(x: __m128i) -> __m128i {
    _mm_or_si128(_mm_srli_epi64(x, 32), _mm_slli_epi64(x, 64 - 32))
}

#[inline(always)]
unsafe fn rot24(x: __m128i) -> __m128i {
    _mm_or_si128(_mm_srli_epi64(x, 24), _mm_slli_epi64(x, 64 - 24))
}

#[inline(always)]
unsafe fn rot16(x: __m128i) -> __m128i {
    _mm_or_si128(_mm_srli_epi64(x, 16), _mm_slli_epi64(x, 64 - 16))
}

#[inline(always)]
unsafe fn rot63(x: __m128i) -> __m128i {
    _mm_or_si128(_mm_srli_epi64(x, 63), _mm_slli_epi64(x, 64 - 63))
}

#[inline(always)]
unsafe fn round(v: &mut [__m128i; 16], m: &[__m128i; 16], r: usize) {
    v[0] = add(v[0], m[SIGMA[r][0] as usize]);
    v[1] = add(v[1], m[SIGMA[r][2] as usize]);
    v[2] = add(v[2], m[SIGMA[r][4] as usize]);
    v[3] = add(v[3], m[SIGMA[r][6] as usize]);
    v[0] = add(v[0], v[4]);
    v[1] = add(v[1], v[5]);
    v[2] = add(v[2], v[6]);
    v[3] = add(v[3], v[7]);
    v[12] = xor(v[12], v[0]);
    v[13] = xor(v[13], v[1]);
    v[14] = xor(v[14], v[2]);
    v[15] = xor(v[15], v[3]);
    v[12] = rot32(v[12]);
    v[13] = rot32(v[13]);
    v[14] = rot32(v[14]);
    v[15] = rot32(v[15]);
    v[8] = add(v[8], v[12]);
    v[9] = add(v[9], v[13]);
    v[10] = add(v[10], v[14]);
    v[11] = add(v[11], v[15]);
    v[4] = xor(v[4], v[8]);
    v[5] = xor(v[5], v[9]);
    v[6] = xor(v[6], v[10]);
    v[7] = xor(v[7], v[11]);
    v[4] = rot24(v[4]);
    v[5] = rot24(v[5]);
    v[6] = rot24(v[6]);
    v[7] = rot24(v[7]);
    v[0] = add(v[0], m[SIGMA[r][1] as usize]);
    v[1] = add(v[1], m[SIGMA[r][3] as usize]);
    v[2] = add(v[2], m[SIGMA[r][5] as usize]);
    v[3] = add(v[3], m[SIGMA[r][7] as usize]);
    v[0] = add(v[0], v[4]);
    v[1] = add(v[1], v[5]);
    v[2] = add(v[2], v[6]);
    v[3] = add(v[3], v[7]);
    v[12] = xor(v[12], v[0]);
    v[13] = xor(v[13], v[1]);
    v[14] = xor(v[14], v[2]);
    v[15] = xor(v[15], v[3]);
    v[12] = rot16(v[12]);
    v[13] = rot16(v[13]);
    v[14] = rot16(v[14]);
    v[15] = rot16(v[15]);
    v[8] = add(v[8], v[12]);
    v[9] = add(v[9], v[13]);
    v[10] = add(v[10], v[14]);
    v[11] = add(v[11], v[15]);
    v[4] = xor(v[4], v[8]);
    v[5] = xor(v[5], v[9]);
    v[6] = xor(v[6], v[10]);
    v[7] = xor(v[7], v[11]);
    v[4] = rot63(v[4]);
    v[5] = rot63(v[5]);
    v[6] = rot63(v[6]);
    v[7] = rot63(v[7]);

    v[0] = add(v[0], m[SIGMA[r][8] as usize]);
    v[1] = add(v[1], m[SIGMA[r][10] as usize]);
    v[2] = add(v[2], m[SIGMA[r][12] as usize]);
    v[3] = add(v[3], m[SIGMA[r][14] as usize]);
    v[0] = add(v[0], v[5]);
    v[1] = add(v[1], v[6]);
    v[2] = add(v[2], v[7]);
    v[3] = add(v[3], v[4]);
    v[15] = xor(v[15], v[0]);
    v[12] = xor(v[12], v[1]);
    v[13] = xor(v[13], v[2]);
    v[14] = xor(v[14], v[3]);
    v[15] = rot32(v[15]);
    v[12] = rot32(v[12]);
    v[13] = rot32(v[13]);
    v[14] = rot32(v[14]);
    v[10] = add(v[10], v[15]);
    v[11] = add(v[11], v[12]);
    v[8] = add(v[8], v[13]);
    v[9] = add(v[9], v[14]);
    v[5] = xor(v[5], v[10]);
    v[6] = xor(v[6], v[11]);
    v[7] = xor(v[7], v[8]);
    v[4] = xor(v[4], v[9]);
    v[5] = rot24(v[5]);
    v[6] = rot24(v[6]);
    v[7] = rot24(v[7]);
    v[4] = rot24(v[4]);
    v[0] = add(v[0], m[SIGMA[r][9] as usize]);
    v[1] = add(v[1], m[SIGMA[r][11] as usize]);
    v[2] = add(v[2], m[SIGMA[r][13] as usize]);
    v[3] = add(v[3], m[SIGMA[r][15] as usize]);
    v[0] = add(v[0], v[5]);
    v[1] = add(v[1], v[6]);
    v[2] = add(v[2], v[7]);
    v[3] = add(v[3], v[4]);
    v[15] = xor(v[15], v[0]);
    v[12] = xor(v[12], v[1]);
    v[13] = xor(v[13], v[2]);
    v[14] = xor(v[14], v[3]);
    v[15] = rot16(v[15]);
    v[12] = rot16(v[12]);
    v[13] = rot16(v[13]);
    v[14] = rot16(v[14]);
    v[10] = add(v[10], v[15]);
    v[11] = add(v[11], v[12]);
    v[8] = add(v[8], v[13]);
    v[9] = add(v[9], v[14]);
    v[5] = xor(v[5], v[10]);
    v[6] = xor(v[6], v[11]);
    v[7] = xor(v[7], v[8]);
    v[4] = xor(v[4], v[9]);
    v[5] = rot63(v[5]);
    v[6] = rot63(v[6]);
    v[7] = rot63(v[7]);
    v[4] = rot63(v[4]);
}

// We'd rather make this a regular function with #[inline(always)], but for
// some reason that blows up compile times by about 10 seconds, at least in
// some cases (BLAKE2b avx2.rs). This macro seems to get the same performance
// result, without the compile time issue.
macro_rules! compress2_transposed {
    (
        $h_vecs:expr,
        $msg_vecs:expr,
        $count_low:expr,
        $count_high:expr,
        $lastblock:expr,
        $lastnode:expr,
    ) => {
        let h_vecs: &mut [__m128i; 8] = $h_vecs;
        let msg_vecs: &[__m128i; 16] = $msg_vecs;
        let count_low: __m128i = $count_low;
        let count_high: __m128i = $count_high;
        let lastblock: __m128i = $lastblock;
        let lastnode: __m128i = $lastnode;
        let mut v = [
            h_vecs[0],
            h_vecs[1],
            h_vecs[2],
            h_vecs[3],
            h_vecs[4],
            h_vecs[5],
            h_vecs[6],
            h_vecs[7],
            set1(IV[0]),
            set1(IV[1]),
            set1(IV[2]),
            set1(IV[3]),
            xor(set1(IV[4]), count_low),
            xor(set1(IV[5]), count_high),
            xor(set1(IV[6]), lastblock),
            xor(set1(IV[7]), lastnode),
        ];

        round(&mut v, &msg_vecs, 0);
        round(&mut v, &msg_vecs, 1);
        round(&mut v, &msg_vecs, 2);
        round(&mut v, &msg_vecs, 3);
        round(&mut v, &msg_vecs, 4);
        round(&mut v, &msg_vecs, 5);
        round(&mut v, &msg_vecs, 6);
        round(&mut v, &msg_vecs, 7);
        round(&mut v, &msg_vecs, 8);
        round(&mut v, &msg_vecs, 9);
        round(&mut v, &msg_vecs, 10);
        round(&mut v, &msg_vecs, 11);

        h_vecs[0] = xor(xor(h_vecs[0], v[0]), v[8]);
        h_vecs[1] = xor(xor(h_vecs[1], v[1]), v[9]);
        h_vecs[2] = xor(xor(h_vecs[2], v[2]), v[10]);
        h_vecs[3] = xor(xor(h_vecs[3], v[3]), v[11]);
        h_vecs[4] = xor(xor(h_vecs[4], v[4]), v[12]);
        h_vecs[5] = xor(xor(h_vecs[5], v[5]), v[13]);
        h_vecs[6] = xor(xor(h_vecs[6], v[6]), v[14]);
        h_vecs[7] = xor(xor(h_vecs[7], v[7]), v[15]);
    };
}

#[inline(always)]
unsafe fn transpose_vecs(a: __m128i, b: __m128i) -> [__m128i; DEGREE] {
    let a_words: [Word; DEGREE] = mem::transmute(a);
    let b_words: [Word; DEGREE] = mem::transmute(b);
    [set2(a_words[0], b_words[0]), set2(a_words[1], b_words[1])]
}

#[inline(always)]
unsafe fn transpose_state_vecs(jobs: &[Job; DEGREE]) -> [__m128i; 8] {
    // Load all the state words into transposed vectors, where the first vector
    // has the first word of each state, etc. Transposing once at the beginning
    // and once at the end is more efficient that repeating it for each block.
    let words0 = array_refs!(&jobs[0].words, DEGREE, DEGREE, DEGREE, DEGREE);
    let words1 = array_refs!(&jobs[1].words, DEGREE, DEGREE, DEGREE, DEGREE);
    let [h0, h1] = transpose_vecs(loadu(words0.0), loadu(words1.0));
    let [h2, h3] = transpose_vecs(loadu(words0.1), loadu(words1.1));
    let [h4, h5] = transpose_vecs(loadu(words0.2), loadu(words1.2));
    let [h6, h7] = transpose_vecs(loadu(words0.3), loadu(words1.3));
    [h0, h1, h2, h3, h4, h5, h6, h7]
}

#[inline(always)]
unsafe fn untranspose_state_vecs(h_vecs: &[__m128i; 8], jobs: &mut [Job; DEGREE]) {
    // Un-transpose the updated state vectors back into the caller's arrays.
    let [job0, job1] = jobs;
    let words0 = mut_array_refs!(&mut job0.words, DEGREE, DEGREE, DEGREE, DEGREE);
    let words1 = mut_array_refs!(&mut job1.words, DEGREE, DEGREE, DEGREE, DEGREE);

    let out = transpose_vecs(h_vecs[0], h_vecs[1]);
    storeu(out[0], words0.0);
    storeu(out[1], words1.0);
    let out = transpose_vecs(h_vecs[2], h_vecs[3]);
    storeu(out[0], words0.1);
    storeu(out[1], words1.1);
    let out = transpose_vecs(h_vecs[4], h_vecs[5]);
    storeu(out[0], words0.2);
    storeu(out[1], words1.2);
    let out = transpose_vecs(h_vecs[6], h_vecs[7]);
    storeu(out[0], words0.3);
    storeu(out[1], words1.3);
}

#[inline(always)]
unsafe fn transpose_msg_vecs(blocks: [*const [u8; BLOCKBYTES]; DEGREE]) -> [__m128i; 16] {
    // These input arrays have no particular alignment, so we use unaligned
    // loads to read from them.
    let block0 = blocks[0] as *const [Word; DEGREE];
    let block1 = blocks[1] as *const [Word; DEGREE];
    let [m0, m1] = transpose_vecs(loadu(block0.add(0)), loadu(block1.add(0)));
    let [m2, m3] = transpose_vecs(loadu(block0.add(1)), loadu(block1.add(1)));
    let [m4, m5] = transpose_vecs(loadu(block0.add(2)), loadu(block1.add(2)));
    let [m6, m7] = transpose_vecs(loadu(block0.add(3)), loadu(block1.add(3)));
    let [m8, m9] = transpose_vecs(loadu(block0.add(4)), loadu(block1.add(4)));
    let [m10, m11] = transpose_vecs(loadu(block0.add(5)), loadu(block1.add(5)));
    let [m12, m13] = transpose_vecs(loadu(block0.add(6)), loadu(block1.add(6)));
    let [m14, m15] = transpose_vecs(loadu(block0.add(7)), loadu(block1.add(7)));
    [
        m0, m1, m2, m3, m4, m5, m6, m7, m8, m9, m10, m11, m12, m13, m14, m15,
    ]
}

#[inline(always)]
unsafe fn load_counts(jobs: &[Job; DEGREE]) -> (__m128i, __m128i) {
    (
        set2(count_low(jobs[0].count), count_low(jobs[1].count)),
        set2(count_high(jobs[0].count), count_high(jobs[1].count)),
    )
}

#[inline(always)]
unsafe fn store_counts(jobs: &mut [Job; DEGREE], low: __m128i, high: __m128i) {
    let low_ints: [Word; DEGREE] = mem::transmute(low);
    let high_ints: [Word; DEGREE] = mem::transmute(high);
    for i in 0..DEGREE {
        jobs[i].count = assemble_count(low_ints[i], high_ints[i]);
    }
}

#[inline(always)]
unsafe fn add_to_counts(lo: &mut __m128i, hi: &mut __m128i, delta: __m128i) {
    // If the low counts reach zero, that means they wrapped, unless the delta
    // was also zero.
    *lo = add(*lo, delta);
    let lo_reached_zero = eq(*lo, set1(0));
    let delta_was_zero = eq(delta, set1(0));
    let hi_inc = and(set1(1), negate_and(delta_was_zero, lo_reached_zero));
    *hi = add(*hi, hi_inc);
}

#[inline(always)]
unsafe fn flags_vec(flags: [bool; DEGREE]) -> __m128i {
    set2(flag_word(flags[0]), flag_word(flags[1]))
}

#[target_feature(enable = "sse4.1")]
pub unsafe fn compress2_loop(jobs: &mut [Job; DEGREE], finalize: Finalize, stride: Stride) {
    // If we're not finalizing, there can't be a partial block at the end.
    for job in jobs.iter() {
        input_debug_asserts(job.input, finalize);
    }

    let msg_ptrs = [jobs[0].input.as_ptr(), jobs[1].input.as_ptr()];
    let mut h_vecs = transpose_state_vecs(&jobs);
    let (mut counts_lo, mut counts_hi) = load_counts(&jobs);

    // Prepare the final blocks (note, which could be empty if the input is
    // empty). Do all this before entering the main loop.
    let min_len = jobs.iter().map(|job| job.input.len()).min().unwrap();
    let mut fin_offset = min_len.saturating_sub(1);
    fin_offset -= fin_offset % stride.padded_blockbytes();
    // Performance note, making these buffers mem::uninitialized() seems to
    // cause problems in the optimizer.
    let mut buf0: [u8; BLOCKBYTES] = [0; BLOCKBYTES];
    let mut buf1: [u8; BLOCKBYTES] = [0; BLOCKBYTES];
    let (block0, len0, finalize0) = final_block(jobs[0].input, fin_offset, &mut buf0, stride);
    let (block1, len1, finalize1) = final_block(jobs[1].input, fin_offset, &mut buf1, stride);
    let fin_blocks: [*const [u8; BLOCKBYTES]; DEGREE] = [block0, block1];
    let fin_counts_delta = set2(len0 as Word, len1 as Word);
    let fin_last_block;
    let fin_last_node;
    if finalize.yes() {
        fin_last_block = flags_vec([finalize0, finalize1]);
        fin_last_node = flags_vec([
            finalize0 && jobs[0].last_node.yes(),
            finalize1 && jobs[1].last_node.yes(),
        ]);
    } else {
        fin_last_block = set1(0);
        fin_last_node = set1(0);
    }

    // The main loop.
    let mut offset = 0;
    loop {
        let blocks;
        let counts_delta;
        let last_block;
        let last_node;
        if offset == fin_offset {
            blocks = fin_blocks;
            counts_delta = fin_counts_delta;
            last_block = fin_last_block;
            last_node = fin_last_node;
        } else {
            blocks = [
                msg_ptrs[0].add(offset) as *const [u8; BLOCKBYTES],
                msg_ptrs[1].add(offset) as *const [u8; BLOCKBYTES],
            ];
            counts_delta = set1(BLOCKBYTES as Word);
            last_block = set1(0);
            last_node = set1(0);
        };

        let m_vecs = transpose_msg_vecs(blocks);
        add_to_counts(&mut counts_lo, &mut counts_hi, counts_delta);
        compress2_transposed!(
            &mut h_vecs,
            &m_vecs,
            counts_lo,
            counts_hi,
            last_block,
            last_node,
        );

        // Check for termination before bumping the offset, to avoid overflow.
        if offset == fin_offset {
            break;
        }

        offset += stride.padded_blockbytes();
    }

    // Write out the results.
    untranspose_state_vecs(&h_vecs, &mut *jobs);
    store_counts(&mut *jobs, counts_lo, counts_hi);
    let max_consumed = offset.saturating_add(stride.padded_blockbytes());
    for job in jobs.iter_mut() {
        let consumed = cmp::min(max_consumed, job.input.len());
        job.input = &job.input[consumed..];
    }
}

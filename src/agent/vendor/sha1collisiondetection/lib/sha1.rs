#![allow(dead_code, mutable_transmutes, non_camel_case_types, non_snake_case,
         non_upper_case_globals, unused_assignments, unused_mut)]

use crate::ubc_check::{sha1_dvs, ubc_check};

pub type __uint32_t = u32; // libc::uint32_t, but that is deprecated.
pub type __uint64_t = u64; // libc::uint64_t, but that is deprecated.
pub type uint32_t = __uint32_t;
pub type uint64_t = __uint64_t;
pub type collision_block_callback
    =
    Option<unsafe extern "C" fn(_: uint64_t, _: *const uint32_t,
                                _: *const uint32_t, _: *const uint32_t,
                                _: *const uint32_t) -> ()>;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct SHA1_CTX {
    pub total: uint64_t,
    pub ihv: [uint32_t; 5],
    pub buffer: [u8; 64],
    pub found_collision: bool,
    pub safe_hash: bool,
    pub detect_coll: bool,
    pub ubc_check: bool,
    pub reduced_round_coll: bool,
    pub callback: collision_block_callback,
    pub ihv1: [uint32_t; 5],
    pub ihv2: [uint32_t; 5],
    pub m1: [uint32_t; 80],
    pub m2: [uint32_t; 80],
    pub states: [[uint32_t; 5]; 80],
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct dv_info_t {
    pub dvType: i32,
    pub dvK: i32,
    pub dvB: i32,
    pub testt: i32,
    pub maski: i32,
    pub maskb: i32,
    pub dm: [uint32_t; 80],
}
unsafe fn memcpy<T>(dst: *mut T, src: *const T, count: usize) {
    core::intrinsics::copy_nonoverlapping(src, dst, count)
}
#[inline]
unsafe extern "C" fn sha1_process_unaligned(mut ctx: *mut SHA1_CTX,
                                            buf: *const core::ffi::c_void) {
    if cfg!(any(target_arch = "x86", target_arch = "x86_64")) {
        sha1_process(ctx, buf as *mut uint32_t as *const uint32_t);
    } else {
        debug_assert_eq!(core::mem::align_of::<u8>(), 1);
        memcpy((*ctx).buffer.as_mut_ptr() as *mut _, buf, 64);
        sha1_process(ctx, (*ctx).buffer.as_mut_ptr() as *const uint32_t);
    }
}
#[inline]
unsafe extern "C" fn rotate_right(mut x: uint32_t, mut n: uint32_t)
 -> uint32_t {
    return x >> n | x << (32 as i32 as u32).wrapping_sub(n);
}
#[inline]
unsafe extern "C" fn rotate_left(mut x: uint32_t, mut n: uint32_t)
 -> uint32_t {
    return x << n | x >> (32 as i32 as u32).wrapping_sub(n);
}
#[inline]
unsafe extern "C" fn sha1_bswap32(mut x: uint32_t) -> uint32_t {
    x =
        x << 8 as i32 & 0xff00ff00 as u32 |
            x >> 8 as i32 & 0xff00ff as i32 as u32;
    x = x << 16 as i32 | x >> 16 as i32;
    return x;
}
#[inline]
unsafe extern "C" fn maybe_bswap32(mut x: uint32_t) -> uint32_t {
    if cfg!(target_endian = "big") {
        x
    } else if cfg!(target_endian = "little") {
        sha1_bswap32(x)
    } else {
        unimplemented!()
    }
}
#[inline]
unsafe extern "C" fn sha1_mix(W: *const uint32_t, mut t: usize) -> uint32_t {
    return rotate_left(*W.offset(t.wrapping_sub(3) as isize) ^
                           *W.offset(t.wrapping_sub(8) as
                                         isize) ^
                           *W.offset(t.wrapping_sub(14) as
                                         isize) ^
                           *W.offset(t.wrapping_sub(16) as
                                         isize),
                       1 as i32 as uint32_t);
}
#[inline]
unsafe extern "C" fn sha1_f1(mut b: uint32_t, mut c: uint32_t,
                             mut d: uint32_t) -> uint32_t {
    return d ^ b & (c ^ d);
}
#[inline]
unsafe extern "C" fn sha1_f2(mut b: uint32_t, mut c: uint32_t,
                             mut d: uint32_t) -> uint32_t {
    return b ^ c ^ d;
}
#[inline]
unsafe extern "C" fn sha1_f3(mut b: uint32_t, mut c: uint32_t,
                             mut d: uint32_t) -> uint32_t {
    return (b & c).wrapping_add(d & (b ^ c));
}
#[inline]
unsafe extern "C" fn sha1_f4(mut b: uint32_t, mut c: uint32_t,
                             mut d: uint32_t) -> uint32_t {
    return b ^ c ^ d;
}
#[inline]
unsafe extern "C" fn hashclash_sha1compress_round1_step(mut a: uint32_t,
                                                        mut b: *mut uint32_t,
                                                        mut c: uint32_t,
                                                        mut d: uint32_t,
                                                        mut e: *mut uint32_t,
                                                        m: *const uint32_t,
                                                        mut t: usize) {
    *e =
        (*e as
             u32).wrapping_add(rotate_left(a,
                                                    5 as i32 as
                                                        uint32_t).wrapping_add(sha1_f1(*b,
                                                                                       c,
                                                                                       d)).wrapping_add(0x5a827999
                                                                                                            as
                                                                                                            i32
                                                                                                            as
                                                                                                            u32).wrapping_add(*m.offset(t
                                                                                                                                                     as
                                                                                                                                                     isize)))
            as uint32_t as uint32_t;
    *b = rotate_left(*b, 30 as i32 as uint32_t);
}
#[inline]
unsafe extern "C" fn hashclash_sha1compress_round2_step(mut a: uint32_t,
                                                        mut b: *mut uint32_t,
                                                        mut c: uint32_t,
                                                        mut d: uint32_t,
                                                        mut e: *mut uint32_t,
                                                        m: *const uint32_t,
                                                        mut t: usize) {
    *e =
        (*e as
             u32).wrapping_add(rotate_left(a,
                                                    5 as i32 as
                                                        uint32_t).wrapping_add(sha1_f2(*b,
                                                                                       c,
                                                                                       d)).wrapping_add(0x6ed9eba1
                                                                                                            as
                                                                                                            i32
                                                                                                            as
                                                                                                            u32).wrapping_add(*m.offset(t
                                                                                                                                                     as
                                                                                                                                                     isize)))
            as uint32_t as uint32_t;
    *b = rotate_left(*b, 30 as i32 as uint32_t);
}
#[inline]
unsafe extern "C" fn hashclash_sha1compress_round3_step(mut a: uint32_t,
                                                        mut b: *mut uint32_t,
                                                        mut c: uint32_t,
                                                        mut d: uint32_t,
                                                        mut e: *mut uint32_t,
                                                        m: *const uint32_t,
                                                        mut t: usize) {
    *e =
        (*e as
             u32).wrapping_add(rotate_left(a,
                                                    5 as i32 as
                                                        uint32_t).wrapping_add(sha1_f3(*b,
                                                                                       c,
                                                                                       d)).wrapping_add(0x8f1bbcdc
                                                                                                            as
                                                                                                            u32).wrapping_add(*m.offset(t
                                                                                                                                                     as
                                                                                                                                                     isize)))
            as uint32_t as uint32_t;
    *b = rotate_left(*b, 30 as i32 as uint32_t);
}
#[inline]
unsafe extern "C" fn hashclash_sha1compress_round4_step(mut a: uint32_t,
                                                        mut b: *mut uint32_t,
                                                        mut c: uint32_t,
                                                        mut d: uint32_t,
                                                        mut e: *mut uint32_t,
                                                        m: *const uint32_t,
                                                        mut t: usize) {
    *e =
        (*e as
             u32).wrapping_add(rotate_left(a,
                                                    5 as i32 as
                                                        uint32_t).wrapping_add(sha1_f4(*b,
                                                                                       c,
                                                                                       d)).wrapping_add(0xca62c1d6
                                                                                                            as
                                                                                                            u32).wrapping_add(*m.offset(t
                                                                                                                                                     as
                                                                                                                                                     isize)))
            as uint32_t as uint32_t;
    *b = rotate_left(*b, 30 as i32 as uint32_t);
}
#[inline]
unsafe extern "C" fn hashclash_sha1compress_round1_step_bw(mut a: uint32_t,
                                                           mut b:
                                                               *mut uint32_t,
                                                           mut c: uint32_t,
                                                           mut d: uint32_t,
                                                           mut e:
                                                               *mut uint32_t,
                                                           m: *const uint32_t,
                                                           mut t: usize) {
    *b = rotate_right(*b, 30 as i32 as uint32_t);
    *e =
        (*e as
             u32).wrapping_sub(rotate_left(a,
                                                    5 as i32 as
                                                        uint32_t).wrapping_add(sha1_f1(*b,
                                                                                       c,
                                                                                       d)).wrapping_add(0x5a827999
                                                                                                            as
                                                                                                            i32
                                                                                                            as
                                                                                                            u32).wrapping_add(*m.offset(t
                                                                                                                                                     as
                                                                                                                                                     isize)))
            as uint32_t as uint32_t;
}
#[inline]
unsafe extern "C" fn hashclash_sha1compress_round2_step_bw(mut a: uint32_t,
                                                           mut b:
                                                               *mut uint32_t,
                                                           mut c: uint32_t,
                                                           mut d: uint32_t,
                                                           mut e:
                                                               *mut uint32_t,
                                                           m: *const uint32_t,
                                                           mut t: usize) {
    *b = rotate_right(*b, 30 as i32 as uint32_t);
    *e =
        (*e as
             u32).wrapping_sub(rotate_left(a,
                                                    5 as i32 as
                                                        uint32_t).wrapping_add(sha1_f2(*b,
                                                                                       c,
                                                                                       d)).wrapping_add(0x6ed9eba1
                                                                                                            as
                                                                                                            i32
                                                                                                            as
                                                                                                            u32).wrapping_add(*m.offset(t
                                                                                                                                                     as
                                                                                                                                                     isize)))
            as uint32_t as uint32_t;
}
#[inline]
unsafe extern "C" fn hashclash_sha1compress_round3_step_bw(mut a: uint32_t,
                                                           mut b:
                                                               *mut uint32_t,
                                                           mut c: uint32_t,
                                                           mut d: uint32_t,
                                                           mut e:
                                                               *mut uint32_t,
                                                           m: *const uint32_t,
                                                           mut t: usize) {
    *b = rotate_right(*b, 30 as i32 as uint32_t);
    *e =
        (*e as
             u32).wrapping_sub(rotate_left(a,
                                                    5 as i32 as
                                                        uint32_t).wrapping_add(sha1_f3(*b,
                                                                                       c,
                                                                                       d)).wrapping_add(0x8f1bbcdc
                                                                                                            as
                                                                                                            u32).wrapping_add(*m.offset(t
                                                                                                                                                     as
                                                                                                                                                     isize)))
            as uint32_t as uint32_t;
}
#[inline]
unsafe extern "C" fn hashclash_sha1compress_round4_step_bw(mut a: uint32_t,
                                                           mut b:
                                                               *mut uint32_t,
                                                           mut c: uint32_t,
                                                           mut d: uint32_t,
                                                           mut e:
                                                               *mut uint32_t,
                                                           m: *const uint32_t,
                                                           mut t: usize) {
    *b = rotate_right(*b, 30 as i32 as uint32_t);
    *e =
        (*e as
             u32).wrapping_sub(rotate_left(a,
                                                    5 as i32 as
                                                        uint32_t).wrapping_add(sha1_f4(*b,
                                                                                       c,
                                                                                       d)).wrapping_add(0xca62c1d6
                                                                                                            as
                                                                                                            u32).wrapping_add(*m.offset(t
                                                                                                                                                     as
                                                                                                                                                     isize)))
            as uint32_t as uint32_t;
}
#[inline]
unsafe extern "C" fn sha1compress_full_round1_step_load(mut a: uint32_t,
                                                        mut b: *mut uint32_t,
                                                        mut c: uint32_t,
                                                        mut d: uint32_t,
                                                        mut e: *mut uint32_t,
                                                        m: *const uint32_t,
                                                        W: *const uint32_t,
                                                        mut t: usize,
                                                        mut temp:
                                                            *mut uint32_t) {
    *temp = maybe_bswap32(*m.offset(t as isize));
    ::core::ptr::write_volatile(&*W.offset(t as isize) as *const uint32_t as
                                    *mut uint32_t, *temp);
    *e =
        (*e as
             u32).wrapping_add((*temp).wrapping_add(rotate_left(a,
                                                                         5 as
                                                                             i32
                                                                             as
                                                                             uint32_t)).wrapping_add(sha1_f1(*b,
                                                                                                             c,
                                                                                                             d)).wrapping_add(0x5a827999
                                                                                                                                  as
                                                                                                                                  i32
                                                                                                                                  as
                                                                                                                                  u32))
            as uint32_t as uint32_t;
    *b = rotate_left(*b, 30 as i32 as uint32_t);
}
#[inline]
unsafe extern "C" fn sha1compress_full_round1_step_expand(mut a: uint32_t,
                                                          mut b:
                                                              *mut uint32_t,
                                                          mut c: uint32_t,
                                                          mut d: uint32_t,
                                                          mut e:
                                                              *mut uint32_t,
                                                          W: *const uint32_t,
                                                          mut t: usize,
                                                          mut temp:
                                                              *mut uint32_t) {
    *temp = sha1_mix(W, t);
    ::core::ptr::write_volatile(&*W.offset(t as isize) as *const uint32_t as
                                    *mut uint32_t, *temp);
    *e =
        (*e as
             u32).wrapping_add((*temp).wrapping_add(rotate_left(a,
                                                                         5 as
                                                                             i32
                                                                             as
                                                                             uint32_t)).wrapping_add(sha1_f1(*b,
                                                                                                             c,
                                                                                                             d)).wrapping_add(0x5a827999
                                                                                                                                  as
                                                                                                                                  i32
                                                                                                                                  as
                                                                                                                                  u32))
            as uint32_t as uint32_t;
    *b = rotate_left(*b, 30 as i32 as uint32_t);
}
#[inline]
unsafe extern "C" fn sha1compress_full_round2_step(mut a: uint32_t,
                                                   mut b: *mut uint32_t,
                                                   mut c: uint32_t,
                                                   mut d: uint32_t,
                                                   mut e: *mut uint32_t,
                                                   W: *const uint32_t,
                                                   mut t: usize,
                                                   mut temp: *mut uint32_t) {
    *temp = sha1_mix(W, t);
    ::core::ptr::write_volatile(&*W.offset(t as isize) as *const uint32_t as
                                    *mut uint32_t, *temp);
    *e =
        (*e as
             u32).wrapping_add((*temp).wrapping_add(rotate_left(a,
                                                                         5 as
                                                                             i32
                                                                             as
                                                                             uint32_t)).wrapping_add(sha1_f2(*b,
                                                                                                             c,
                                                                                                             d)).wrapping_add(0x6ed9eba1
                                                                                                                                  as
                                                                                                                                  i32
                                                                                                                                  as
                                                                                                                                  u32))
            as uint32_t as uint32_t;
    *b = rotate_left(*b, 30 as i32 as uint32_t);
}
#[inline]
unsafe extern "C" fn sha1compress_full_round3_step(mut a: uint32_t,
                                                   mut b: *mut uint32_t,
                                                   mut c: uint32_t,
                                                   mut d: uint32_t,
                                                   mut e: *mut uint32_t,
                                                   W: *const uint32_t,
                                                   mut t: usize,
                                                   mut temp: *mut uint32_t) {
    *temp = sha1_mix(W, t);
    ::core::ptr::write_volatile(&*W.offset(t as isize) as *const uint32_t as
                                    *mut uint32_t, *temp);
    *e =
        (*e as
             u32).wrapping_add((*temp).wrapping_add(rotate_left(a,
                                                                         5 as
                                                                             i32
                                                                             as
                                                                             uint32_t)).wrapping_add(sha1_f3(*b,
                                                                                                             c,
                                                                                                             d)).wrapping_add(0x8f1bbcdc
                                                                                                                                  as
                                                                                                                                  u32))
            as uint32_t as uint32_t;
    *b = rotate_left(*b, 30 as i32 as uint32_t);
}
#[inline]
unsafe extern "C" fn sha1compress_full_round4_step(mut a: uint32_t,
                                                   mut b: *mut uint32_t,
                                                   mut c: uint32_t,
                                                   mut d: uint32_t,
                                                   mut e: *mut uint32_t,
                                                   W: *const uint32_t,
                                                   mut t: usize,
                                                   mut temp: *mut uint32_t) {
    *temp = sha1_mix(W, t);
    ::core::ptr::write_volatile(&*W.offset(t as isize) as *const uint32_t as
                                    *mut uint32_t, *temp);
    *e =
        (*e as
             u32).wrapping_add((*temp).wrapping_add(rotate_left(a,
                                                                         5 as
                                                                             i32
                                                                             as
                                                                             uint32_t)).wrapping_add(sha1_f4(*b,
                                                                                                             c,
                                                                                                             d)).wrapping_add(0xca62c1d6
                                                                                                                                  as
                                                                                                                                  u32))
            as uint32_t as uint32_t;
    *b = rotate_left(*b, 30 as i32 as uint32_t);
}
/*BUILDNOCOLLDETECTSHA1COMPRESSION*/
unsafe extern "C" fn sha1_compression_W(mut ihv: *mut uint32_t,
                                        mut W: *const uint32_t) {
    let mut a: uint32_t = *ihv.offset(0 as i32 as isize);
    let mut b: uint32_t = *ihv.offset(1 as i32 as isize);
    let mut c: uint32_t = *ihv.offset(2 as i32 as isize);
    let mut d: uint32_t = *ihv.offset(3 as i32 as isize);
    let mut e: uint32_t = *ihv.offset(4 as i32 as isize);
    hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, W,
                                       0 as i32 as usize);
    hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, W,
                                       1 as i32 as usize);
    hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, W,
                                       2 as i32 as usize);
    hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, W,
                                       3 as i32 as usize);
    hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, W,
                                       4 as i32 as usize);
    hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, W,
                                       5 as i32 as usize);
    hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, W,
                                       6 as i32 as usize);
    hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, W,
                                       7 as i32 as usize);
    hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, W,
                                       8 as i32 as usize);
    hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, W,
                                       9 as i32 as usize);
    hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, W,
                                       10 as i32 as usize);
    hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, W,
                                       11 as i32 as usize);
    hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, W,
                                       12 as i32 as usize);
    hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, W,
                                       13 as i32 as usize);
    hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, W,
                                       14 as i32 as usize);
    hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, W,
                                       15 as i32 as usize);
    hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, W,
                                       16 as i32 as usize);
    hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, W,
                                       17 as i32 as usize);
    hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, W,
                                       18 as i32 as usize);
    hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, W,
                                       19 as i32 as usize);
    hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, W,
                                       20 as i32 as usize);
    hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, W,
                                       21 as i32 as usize);
    hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, W,
                                       22 as i32 as usize);
    hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, W,
                                       23 as i32 as usize);
    hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, W,
                                       24 as i32 as usize);
    hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, W,
                                       25 as i32 as usize);
    hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, W,
                                       26 as i32 as usize);
    hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, W,
                                       27 as i32 as usize);
    hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, W,
                                       28 as i32 as usize);
    hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, W,
                                       29 as i32 as usize);
    hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, W,
                                       30 as i32 as usize);
    hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, W,
                                       31 as i32 as usize);
    hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, W,
                                       32 as i32 as usize);
    hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, W,
                                       33 as i32 as usize);
    hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, W,
                                       34 as i32 as usize);
    hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, W,
                                       35 as i32 as usize);
    hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, W,
                                       36 as i32 as usize);
    hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, W,
                                       37 as i32 as usize);
    hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, W,
                                       38 as i32 as usize);
    hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, W,
                                       39 as i32 as usize);
    hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, W,
                                       40 as i32 as usize);
    hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, W,
                                       41 as i32 as usize);
    hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, W,
                                       42 as i32 as usize);
    hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, W,
                                       43 as i32 as usize);
    hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, W,
                                       44 as i32 as usize);
    hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, W,
                                       45 as i32 as usize);
    hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, W,
                                       46 as i32 as usize);
    hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, W,
                                       47 as i32 as usize);
    hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, W,
                                       48 as i32 as usize);
    hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, W,
                                       49 as i32 as usize);
    hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, W,
                                       50 as i32 as usize);
    hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, W,
                                       51 as i32 as usize);
    hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, W,
                                       52 as i32 as usize);
    hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, W,
                                       53 as i32 as usize);
    hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, W,
                                       54 as i32 as usize);
    hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, W,
                                       55 as i32 as usize);
    hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, W,
                                       56 as i32 as usize);
    hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, W,
                                       57 as i32 as usize);
    hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, W,
                                       58 as i32 as usize);
    hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, W,
                                       59 as i32 as usize);
    hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, W,
                                       60 as i32 as usize);
    hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, W,
                                       61 as i32 as usize);
    hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, W,
                                       62 as i32 as usize);
    hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, W,
                                       63 as i32 as usize);
    hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, W,
                                       64 as i32 as usize);
    hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, W,
                                       65 as i32 as usize);
    hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, W,
                                       66 as i32 as usize);
    hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, W,
                                       67 as i32 as usize);
    hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, W,
                                       68 as i32 as usize);
    hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, W,
                                       69 as i32 as usize);
    hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, W,
                                       70 as i32 as usize);
    hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, W,
                                       71 as i32 as usize);
    hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, W,
                                       72 as i32 as usize);
    hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, W,
                                       73 as i32 as usize);
    hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, W,
                                       74 as i32 as usize);
    hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, W,
                                       75 as i32 as usize);
    hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, W,
                                       76 as i32 as usize);
    hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, W,
                                       77 as i32 as usize);
    hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, W,
                                       78 as i32 as usize);
    hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, W,
                                       79 as i32 as usize);
    let ref mut fresh0 = *ihv.offset(0 as i32 as isize);
    *fresh0 =
        (*fresh0 as u32).wrapping_add(a) as uint32_t as uint32_t;
    let ref mut fresh1 = *ihv.offset(1 as i32 as isize);
    *fresh1 =
        (*fresh1 as u32).wrapping_add(b) as uint32_t as uint32_t;
    let ref mut fresh2 = *ihv.offset(2 as i32 as isize);
    *fresh2 =
        (*fresh2 as u32).wrapping_add(c) as uint32_t as uint32_t;
    let ref mut fresh3 = *ihv.offset(3 as i32 as isize);
    *fresh3 =
        (*fresh3 as u32).wrapping_add(d) as uint32_t as uint32_t;
    let ref mut fresh4 = *ihv.offset(4 as i32 as isize);
    *fresh4 =
        (*fresh4 as u32).wrapping_add(e) as uint32_t as uint32_t;
}
unsafe fn sha1_compression_states(mut ihv: *mut uint32_t,
                                                 mut m: *const uint32_t,
                                                 mut W: *mut uint32_t,
                                                 mut states:
                                                     *mut [uint32_t; 5]) {
    let mut a: uint32_t = *ihv.offset(0 as i32 as isize);
    let mut b: uint32_t = *ihv.offset(1 as i32 as isize);
    let mut c: uint32_t = *ihv.offset(2 as i32 as isize);
    let mut d: uint32_t = *ihv.offset(3 as i32 as isize);
    let mut e: uint32_t = *ihv.offset(4 as i32 as isize);
    let mut temp: uint32_t = 0;
    sha1compress_full_round1_step_load(a, &mut b, c, d, &mut e, m,
                                       W as *const uint32_t,
                                       0 as i32 as usize, &mut temp);
    sha1compress_full_round1_step_load(e, &mut a, b, c, &mut d, m,
                                       W as *const uint32_t,
                                       1 as i32 as usize, &mut temp);
    sha1compress_full_round1_step_load(d, &mut e, a, b, &mut c, m,
                                       W as *const uint32_t,
                                       2 as i32 as usize, &mut temp);
    sha1compress_full_round1_step_load(c, &mut d, e, a, &mut b, m,
                                       W as *const uint32_t,
                                       3 as i32 as usize, &mut temp);
    sha1compress_full_round1_step_load(b, &mut c, d, e, &mut a, m,
                                       W as *const uint32_t,
                                       4 as i32 as usize, &mut temp);
    sha1compress_full_round1_step_load(a, &mut b, c, d, &mut e, m,
                                       W as *const uint32_t,
                                       5 as i32 as usize, &mut temp);
    sha1compress_full_round1_step_load(e, &mut a, b, c, &mut d, m,
                                       W as *const uint32_t,
                                       6 as i32 as usize, &mut temp);
    sha1compress_full_round1_step_load(d, &mut e, a, b, &mut c, m,
                                       W as *const uint32_t,
                                       7 as i32 as usize, &mut temp);
    sha1compress_full_round1_step_load(c, &mut d, e, a, &mut b, m,
                                       W as *const uint32_t,
                                       8 as i32 as usize, &mut temp);
    sha1compress_full_round1_step_load(b, &mut c, d, e, &mut a, m,
                                       W as *const uint32_t,
                                       9 as i32 as usize, &mut temp);
    sha1compress_full_round1_step_load(a, &mut b, c, d, &mut e, m,
                                       W as *const uint32_t,
                                       10 as i32 as usize,
                                       &mut temp);
    sha1compress_full_round1_step_load(e, &mut a, b, c, &mut d, m,
                                       W as *const uint32_t,
                                       11 as i32 as usize,
                                       &mut temp);
    sha1compress_full_round1_step_load(d, &mut e, a, b, &mut c, m,
                                       W as *const uint32_t,
                                       12 as i32 as usize,
                                       &mut temp);
    sha1compress_full_round1_step_load(c, &mut d, e, a, &mut b, m,
                                       W as *const uint32_t,
                                       13 as i32 as usize,
                                       &mut temp);
    sha1compress_full_round1_step_load(b, &mut c, d, e, &mut a, m,
                                       W as *const uint32_t,
                                       14 as i32 as usize,
                                       &mut temp);
    sha1compress_full_round1_step_load(a, &mut b, c, d, &mut e, m,
                                       W as *const uint32_t,
                                       15 as i32 as usize,
                                       &mut temp);
    sha1compress_full_round1_step_expand(e, &mut a, b, c, &mut d,
                                         W as *const uint32_t,
                                         16 as i32 as usize,
                                         &mut temp);
    sha1compress_full_round1_step_expand(d, &mut e, a, b, &mut c,
                                         W as *const uint32_t,
                                         17 as i32 as usize,
                                         &mut temp);
    sha1compress_full_round1_step_expand(c, &mut d, e, a, &mut b,
                                         W as *const uint32_t,
                                         18 as i32 as usize,
                                         &mut temp);
    sha1compress_full_round1_step_expand(b, &mut c, d, e, &mut a,
                                         W as *const uint32_t,
                                         19 as i32 as usize,
                                         &mut temp);
    sha1compress_full_round2_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  20 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  21 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  22 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  23 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  24 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  25 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  26 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  27 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  28 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  29 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  30 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  31 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  32 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  33 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  34 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  35 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  36 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  37 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  38 as i32 as usize, &mut temp);
    sha1compress_full_round2_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  39 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  40 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  41 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  42 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  43 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  44 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  45 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  46 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  47 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  48 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  49 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  50 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  51 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  52 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  53 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  54 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  55 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  56 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  57 as i32 as usize, &mut temp);
    (*states.offset(58 as i32 as isize))[0 as i32 as usize] =
        a;
    (*states.offset(58 as i32 as isize))[1 as i32 as usize] =
        b;
    (*states.offset(58 as i32 as isize))[2 as i32 as usize] =
        c;
    (*states.offset(58 as i32 as isize))[3 as i32 as usize] =
        d;
    (*states.offset(58 as i32 as isize))[4 as i32 as usize] =
        e;
    sha1compress_full_round3_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  58 as i32 as usize, &mut temp);
    sha1compress_full_round3_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  59 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  60 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  61 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  62 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  63 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  64 as i32 as usize, &mut temp);
    (*states.offset(65 as i32 as isize))[0 as i32 as usize] =
        a;
    (*states.offset(65 as i32 as isize))[1 as i32 as usize] =
        b;
    (*states.offset(65 as i32 as isize))[2 as i32 as usize] =
        c;
    (*states.offset(65 as i32 as isize))[3 as i32 as usize] =
        d;
    (*states.offset(65 as i32 as isize))[4 as i32 as usize] =
        e;
    sha1compress_full_round4_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  65 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  66 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  67 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  68 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  69 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  70 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  71 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  72 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  73 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  74 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(a, &mut b, c, d, &mut e,
                                  W as *const uint32_t,
                                  75 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(e, &mut a, b, c, &mut d,
                                  W as *const uint32_t,
                                  76 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(d, &mut e, a, b, &mut c,
                                  W as *const uint32_t,
                                  77 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(c, &mut d, e, a, &mut b,
                                  W as *const uint32_t,
                                  78 as i32 as usize, &mut temp);
    sha1compress_full_round4_step(b, &mut c, d, e, &mut a,
                                  W as *const uint32_t,
                                  79 as i32 as usize, &mut temp);
    let ref mut fresh5 = *ihv.offset(0 as i32 as isize);
    *fresh5 =
        (*fresh5 as u32).wrapping_add(a) as uint32_t as uint32_t;
    let ref mut fresh6 = *ihv.offset(1 as i32 as isize);
    *fresh6 =
        (*fresh6 as u32).wrapping_add(b) as uint32_t as uint32_t;
    let ref mut fresh7 = *ihv.offset(2 as i32 as isize);
    *fresh7 =
        (*fresh7 as u32).wrapping_add(c) as uint32_t as uint32_t;
    let ref mut fresh8 = *ihv.offset(3 as i32 as isize);
    *fresh8 =
        (*fresh8 as u32).wrapping_add(d) as uint32_t as uint32_t;
    let ref mut fresh9 = *ihv.offset(4 as i32 as isize);
    *fresh9 =
        (*fresh9 as u32).wrapping_add(e) as uint32_t as uint32_t;
}
unsafe extern "C" fn sha1recompress_fast_58(mut ihvin: *mut uint32_t,
                                            mut ihvout: *mut uint32_t,
                                            mut me2: *const uint32_t,
                                            mut state: *const uint32_t) {
    let mut a: uint32_t = *state.offset(0 as i32 as isize);
    let mut b: uint32_t = *state.offset(1 as i32 as isize);
    let mut c: uint32_t = *state.offset(2 as i32 as isize);
    let mut d: uint32_t = *state.offset(3 as i32 as isize);
    let mut e: uint32_t = *state.offset(4 as i32 as isize);
    if 58 as i32 > 79 as i32 {
        hashclash_sha1compress_round4_step_bw(b, &mut c, d, e, &mut a, me2,
                                              79 as i32 as usize);
    }
    if 58 as i32 > 78 as i32 {
        hashclash_sha1compress_round4_step_bw(c, &mut d, e, a, &mut b, me2,
                                              78 as i32 as usize);
    }
    if 58 as i32 > 77 as i32 {
        hashclash_sha1compress_round4_step_bw(d, &mut e, a, b, &mut c, me2,
                                              77 as i32 as usize);
    }
    if 58 as i32 > 76 as i32 {
        hashclash_sha1compress_round4_step_bw(e, &mut a, b, c, &mut d, me2,
                                              76 as i32 as usize);
    }
    if 58 as i32 > 75 as i32 {
        hashclash_sha1compress_round4_step_bw(a, &mut b, c, d, &mut e, me2,
                                              75 as i32 as usize);
    }
    if 58 as i32 > 74 as i32 {
        hashclash_sha1compress_round4_step_bw(b, &mut c, d, e, &mut a, me2,
                                              74 as i32 as usize);
    }
    if 58 as i32 > 73 as i32 {
        hashclash_sha1compress_round4_step_bw(c, &mut d, e, a, &mut b, me2,
                                              73 as i32 as usize);
    }
    if 58 as i32 > 72 as i32 {
        hashclash_sha1compress_round4_step_bw(d, &mut e, a, b, &mut c, me2,
                                              72 as i32 as usize);
    }
    if 58 as i32 > 71 as i32 {
        hashclash_sha1compress_round4_step_bw(e, &mut a, b, c, &mut d, me2,
                                              71 as i32 as usize);
    }
    if 58 as i32 > 70 as i32 {
        hashclash_sha1compress_round4_step_bw(a, &mut b, c, d, &mut e, me2,
                                              70 as i32 as usize);
    }
    if 58 as i32 > 69 as i32 {
        hashclash_sha1compress_round4_step_bw(b, &mut c, d, e, &mut a, me2,
                                              69 as i32 as usize);
    }
    if 58 as i32 > 68 as i32 {
        hashclash_sha1compress_round4_step_bw(c, &mut d, e, a, &mut b, me2,
                                              68 as i32 as usize);
    }
    if 58 as i32 > 67 as i32 {
        hashclash_sha1compress_round4_step_bw(d, &mut e, a, b, &mut c, me2,
                                              67 as i32 as usize);
    }
    if 58 as i32 > 66 as i32 {
        hashclash_sha1compress_round4_step_bw(e, &mut a, b, c, &mut d, me2,
                                              66 as i32 as usize);
    }
    if 58 as i32 > 65 as i32 {
        hashclash_sha1compress_round4_step_bw(a, &mut b, c, d, &mut e, me2,
                                              65 as i32 as usize);
    }
    if 58 as i32 > 64 as i32 {
        hashclash_sha1compress_round4_step_bw(b, &mut c, d, e, &mut a, me2,
                                              64 as i32 as usize);
    }
    if 58 as i32 > 63 as i32 {
        hashclash_sha1compress_round4_step_bw(c, &mut d, e, a, &mut b, me2,
                                              63 as i32 as usize);
    }
    if 58 as i32 > 62 as i32 {
        hashclash_sha1compress_round4_step_bw(d, &mut e, a, b, &mut c, me2,
                                              62 as i32 as usize);
    }
    if 58 as i32 > 61 as i32 {
        hashclash_sha1compress_round4_step_bw(e, &mut a, b, c, &mut d, me2,
                                              61 as i32 as usize);
    }
    if 58 as i32 > 60 as i32 {
        hashclash_sha1compress_round4_step_bw(a, &mut b, c, d, &mut e, me2,
                                              60 as i32 as usize);
    }
    if 58 as i32 > 59 as i32 {
        hashclash_sha1compress_round3_step_bw(b, &mut c, d, e, &mut a, me2,
                                              59 as i32 as usize);
    }
    if 58 as i32 > 58 as i32 {
        hashclash_sha1compress_round3_step_bw(c, &mut d, e, a, &mut b, me2,
                                              58 as i32 as usize);
    }
    if 58 as i32 > 57 as i32 {
        hashclash_sha1compress_round3_step_bw(d, &mut e, a, b, &mut c, me2,
                                              57 as i32 as usize);
    }
    if 58 as i32 > 56 as i32 {
        hashclash_sha1compress_round3_step_bw(e, &mut a, b, c, &mut d, me2,
                                              56 as i32 as usize);
    }
    if 58 as i32 > 55 as i32 {
        hashclash_sha1compress_round3_step_bw(a, &mut b, c, d, &mut e, me2,
                                              55 as i32 as usize);
    }
    if 58 as i32 > 54 as i32 {
        hashclash_sha1compress_round3_step_bw(b, &mut c, d, e, &mut a, me2,
                                              54 as i32 as usize);
    }
    if 58 as i32 > 53 as i32 {
        hashclash_sha1compress_round3_step_bw(c, &mut d, e, a, &mut b, me2,
                                              53 as i32 as usize);
    }
    if 58 as i32 > 52 as i32 {
        hashclash_sha1compress_round3_step_bw(d, &mut e, a, b, &mut c, me2,
                                              52 as i32 as usize);
    }
    if 58 as i32 > 51 as i32 {
        hashclash_sha1compress_round3_step_bw(e, &mut a, b, c, &mut d, me2,
                                              51 as i32 as usize);
    }
    if 58 as i32 > 50 as i32 {
        hashclash_sha1compress_round3_step_bw(a, &mut b, c, d, &mut e, me2,
                                              50 as i32 as usize);
    }
    if 58 as i32 > 49 as i32 {
        hashclash_sha1compress_round3_step_bw(b, &mut c, d, e, &mut a, me2,
                                              49 as i32 as usize);
    }
    if 58 as i32 > 48 as i32 {
        hashclash_sha1compress_round3_step_bw(c, &mut d, e, a, &mut b, me2,
                                              48 as i32 as usize);
    }
    if 58 as i32 > 47 as i32 {
        hashclash_sha1compress_round3_step_bw(d, &mut e, a, b, &mut c, me2,
                                              47 as i32 as usize);
    }
    if 58 as i32 > 46 as i32 {
        hashclash_sha1compress_round3_step_bw(e, &mut a, b, c, &mut d, me2,
                                              46 as i32 as usize);
    }
    if 58 as i32 > 45 as i32 {
        hashclash_sha1compress_round3_step_bw(a, &mut b, c, d, &mut e, me2,
                                              45 as i32 as usize);
    }
    if 58 as i32 > 44 as i32 {
        hashclash_sha1compress_round3_step_bw(b, &mut c, d, e, &mut a, me2,
                                              44 as i32 as usize);
    }
    if 58 as i32 > 43 as i32 {
        hashclash_sha1compress_round3_step_bw(c, &mut d, e, a, &mut b, me2,
                                              43 as i32 as usize);
    }
    if 58 as i32 > 42 as i32 {
        hashclash_sha1compress_round3_step_bw(d, &mut e, a, b, &mut c, me2,
                                              42 as i32 as usize);
    }
    if 58 as i32 > 41 as i32 {
        hashclash_sha1compress_round3_step_bw(e, &mut a, b, c, &mut d, me2,
                                              41 as i32 as usize);
    }
    if 58 as i32 > 40 as i32 {
        hashclash_sha1compress_round3_step_bw(a, &mut b, c, d, &mut e, me2,
                                              40 as i32 as usize);
    }
    if 58 as i32 > 39 as i32 {
        hashclash_sha1compress_round2_step_bw(b, &mut c, d, e, &mut a, me2,
                                              39 as i32 as usize);
    }
    if 58 as i32 > 38 as i32 {
        hashclash_sha1compress_round2_step_bw(c, &mut d, e, a, &mut b, me2,
                                              38 as i32 as usize);
    }
    if 58 as i32 > 37 as i32 {
        hashclash_sha1compress_round2_step_bw(d, &mut e, a, b, &mut c, me2,
                                              37 as i32 as usize);
    }
    if 58 as i32 > 36 as i32 {
        hashclash_sha1compress_round2_step_bw(e, &mut a, b, c, &mut d, me2,
                                              36 as i32 as usize);
    }
    if 58 as i32 > 35 as i32 {
        hashclash_sha1compress_round2_step_bw(a, &mut b, c, d, &mut e, me2,
                                              35 as i32 as usize);
    }
    if 58 as i32 > 34 as i32 {
        hashclash_sha1compress_round2_step_bw(b, &mut c, d, e, &mut a, me2,
                                              34 as i32 as usize);
    }
    if 58 as i32 > 33 as i32 {
        hashclash_sha1compress_round2_step_bw(c, &mut d, e, a, &mut b, me2,
                                              33 as i32 as usize);
    }
    if 58 as i32 > 32 as i32 {
        hashclash_sha1compress_round2_step_bw(d, &mut e, a, b, &mut c, me2,
                                              32 as i32 as usize);
    }
    if 58 as i32 > 31 as i32 {
        hashclash_sha1compress_round2_step_bw(e, &mut a, b, c, &mut d, me2,
                                              31 as i32 as usize);
    }
    if 58 as i32 > 30 as i32 {
        hashclash_sha1compress_round2_step_bw(a, &mut b, c, d, &mut e, me2,
                                              30 as i32 as usize);
    }
    if 58 as i32 > 29 as i32 {
        hashclash_sha1compress_round2_step_bw(b, &mut c, d, e, &mut a, me2,
                                              29 as i32 as usize);
    }
    if 58 as i32 > 28 as i32 {
        hashclash_sha1compress_round2_step_bw(c, &mut d, e, a, &mut b, me2,
                                              28 as i32 as usize);
    }
    if 58 as i32 > 27 as i32 {
        hashclash_sha1compress_round2_step_bw(d, &mut e, a, b, &mut c, me2,
                                              27 as i32 as usize);
    }
    if 58 as i32 > 26 as i32 {
        hashclash_sha1compress_round2_step_bw(e, &mut a, b, c, &mut d, me2,
                                              26 as i32 as usize);
    }
    if 58 as i32 > 25 as i32 {
        hashclash_sha1compress_round2_step_bw(a, &mut b, c, d, &mut e, me2,
                                              25 as i32 as usize);
    }
    if 58 as i32 > 24 as i32 {
        hashclash_sha1compress_round2_step_bw(b, &mut c, d, e, &mut a, me2,
                                              24 as i32 as usize);
    }
    if 58 as i32 > 23 as i32 {
        hashclash_sha1compress_round2_step_bw(c, &mut d, e, a, &mut b, me2,
                                              23 as i32 as usize);
    }
    if 58 as i32 > 22 as i32 {
        hashclash_sha1compress_round2_step_bw(d, &mut e, a, b, &mut c, me2,
                                              22 as i32 as usize);
    }
    if 58 as i32 > 21 as i32 {
        hashclash_sha1compress_round2_step_bw(e, &mut a, b, c, &mut d, me2,
                                              21 as i32 as usize);
    }
    if 58 as i32 > 20 as i32 {
        hashclash_sha1compress_round2_step_bw(a, &mut b, c, d, &mut e, me2,
                                              20 as i32 as usize);
    }
    if 58 as i32 > 19 as i32 {
        hashclash_sha1compress_round1_step_bw(b, &mut c, d, e, &mut a, me2,
                                              19 as i32 as usize);
    }
    if 58 as i32 > 18 as i32 {
        hashclash_sha1compress_round1_step_bw(c, &mut d, e, a, &mut b, me2,
                                              18 as i32 as usize);
    }
    if 58 as i32 > 17 as i32 {
        hashclash_sha1compress_round1_step_bw(d, &mut e, a, b, &mut c, me2,
                                              17 as i32 as usize);
    }
    if 58 as i32 > 16 as i32 {
        hashclash_sha1compress_round1_step_bw(e, &mut a, b, c, &mut d, me2,
                                              16 as i32 as usize);
    }
    if 58 as i32 > 15 as i32 {
        hashclash_sha1compress_round1_step_bw(a, &mut b, c, d, &mut e, me2,
                                              15 as i32 as usize);
    }
    if 58 as i32 > 14 as i32 {
        hashclash_sha1compress_round1_step_bw(b, &mut c, d, e, &mut a, me2,
                                              14 as i32 as usize);
    }
    if 58 as i32 > 13 as i32 {
        hashclash_sha1compress_round1_step_bw(c, &mut d, e, a, &mut b, me2,
                                              13 as i32 as usize);
    }
    if 58 as i32 > 12 as i32 {
        hashclash_sha1compress_round1_step_bw(d, &mut e, a, b, &mut c, me2,
                                              12 as i32 as usize);
    }
    if 58 as i32 > 11 as i32 {
        hashclash_sha1compress_round1_step_bw(e, &mut a, b, c, &mut d, me2,
                                              11 as i32 as usize);
    }
    if 58 as i32 > 10 as i32 {
        hashclash_sha1compress_round1_step_bw(a, &mut b, c, d, &mut e, me2,
                                              10 as i32 as usize);
    }
    if 58 as i32 > 9 as i32 {
        hashclash_sha1compress_round1_step_bw(b, &mut c, d, e, &mut a, me2,
                                              9 as i32 as usize);
    }
    if 58 as i32 > 8 as i32 {
        hashclash_sha1compress_round1_step_bw(c, &mut d, e, a, &mut b, me2,
                                              8 as i32 as usize);
    }
    if 58 as i32 > 7 as i32 {
        hashclash_sha1compress_round1_step_bw(d, &mut e, a, b, &mut c, me2,
                                              7 as i32 as usize);
    }
    if 58 as i32 > 6 as i32 {
        hashclash_sha1compress_round1_step_bw(e, &mut a, b, c, &mut d, me2,
                                              6 as i32 as usize);
    }
    if 58 as i32 > 5 as i32 {
        hashclash_sha1compress_round1_step_bw(a, &mut b, c, d, &mut e, me2,
                                              5 as i32 as usize);
    }
    if 58 as i32 > 4 as i32 {
        hashclash_sha1compress_round1_step_bw(b, &mut c, d, e, &mut a, me2,
                                              4 as i32 as usize);
    }
    if 58 as i32 > 3 as i32 {
        hashclash_sha1compress_round1_step_bw(c, &mut d, e, a, &mut b, me2,
                                              3 as i32 as usize);
    }
    if 58 as i32 > 2 as i32 {
        hashclash_sha1compress_round1_step_bw(d, &mut e, a, b, &mut c, me2,
                                              2 as i32 as usize);
    }
    if 58 as i32 > 1 as i32 {
        hashclash_sha1compress_round1_step_bw(e, &mut a, b, c, &mut d, me2,
                                              1 as i32 as usize);
    }
    if 58 as i32 > 0 as i32 {
        hashclash_sha1compress_round1_step_bw(a, &mut b, c, d, &mut e, me2,
                                              0 as i32 as usize);
    }
    *ihvin.offset(0 as i32 as isize) = a;
    *ihvin.offset(1 as i32 as isize) = b;
    *ihvin.offset(2 as i32 as isize) = c;
    *ihvin.offset(3 as i32 as isize) = d;
    *ihvin.offset(4 as i32 as isize) = e;
    a = *state.offset(0 as i32 as isize);
    b = *state.offset(1 as i32 as isize);
    c = *state.offset(2 as i32 as isize);
    d = *state.offset(3 as i32 as isize);
    e = *state.offset(4 as i32 as isize);
    if 58 as i32 <= 0 as i32 {
        hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, me2,
                                           0 as i32 as usize);
    }
    if 58 as i32 <= 1 as i32 {
        hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, me2,
                                           1 as i32 as usize);
    }
    if 58 as i32 <= 2 as i32 {
        hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, me2,
                                           2 as i32 as usize);
    }
    if 58 as i32 <= 3 as i32 {
        hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, me2,
                                           3 as i32 as usize);
    }
    if 58 as i32 <= 4 as i32 {
        hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, me2,
                                           4 as i32 as usize);
    }
    if 58 as i32 <= 5 as i32 {
        hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, me2,
                                           5 as i32 as usize);
    }
    if 58 as i32 <= 6 as i32 {
        hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, me2,
                                           6 as i32 as usize);
    }
    if 58 as i32 <= 7 as i32 {
        hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, me2,
                                           7 as i32 as usize);
    }
    if 58 as i32 <= 8 as i32 {
        hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, me2,
                                           8 as i32 as usize);
    }
    if 58 as i32 <= 9 as i32 {
        hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, me2,
                                           9 as i32 as usize);
    }
    if 58 as i32 <= 10 as i32 {
        hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, me2,
                                           10 as i32 as usize);
    }
    if 58 as i32 <= 11 as i32 {
        hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, me2,
                                           11 as i32 as usize);
    }
    if 58 as i32 <= 12 as i32 {
        hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, me2,
                                           12 as i32 as usize);
    }
    if 58 as i32 <= 13 as i32 {
        hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, me2,
                                           13 as i32 as usize);
    }
    if 58 as i32 <= 14 as i32 {
        hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, me2,
                                           14 as i32 as usize);
    }
    if 58 as i32 <= 15 as i32 {
        hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, me2,
                                           15 as i32 as usize);
    }
    if 58 as i32 <= 16 as i32 {
        hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, me2,
                                           16 as i32 as usize);
    }
    if 58 as i32 <= 17 as i32 {
        hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, me2,
                                           17 as i32 as usize);
    }
    if 58 as i32 <= 18 as i32 {
        hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, me2,
                                           18 as i32 as usize);
    }
    if 58 as i32 <= 19 as i32 {
        hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, me2,
                                           19 as i32 as usize);
    }
    if 58 as i32 <= 20 as i32 {
        hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, me2,
                                           20 as i32 as usize);
    }
    if 58 as i32 <= 21 as i32 {
        hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, me2,
                                           21 as i32 as usize);
    }
    if 58 as i32 <= 22 as i32 {
        hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, me2,
                                           22 as i32 as usize);
    }
    if 58 as i32 <= 23 as i32 {
        hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, me2,
                                           23 as i32 as usize);
    }
    if 58 as i32 <= 24 as i32 {
        hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, me2,
                                           24 as i32 as usize);
    }
    if 58 as i32 <= 25 as i32 {
        hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, me2,
                                           25 as i32 as usize);
    }
    if 58 as i32 <= 26 as i32 {
        hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, me2,
                                           26 as i32 as usize);
    }
    if 58 as i32 <= 27 as i32 {
        hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, me2,
                                           27 as i32 as usize);
    }
    if 58 as i32 <= 28 as i32 {
        hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, me2,
                                           28 as i32 as usize);
    }
    if 58 as i32 <= 29 as i32 {
        hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, me2,
                                           29 as i32 as usize);
    }
    if 58 as i32 <= 30 as i32 {
        hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, me2,
                                           30 as i32 as usize);
    }
    if 58 as i32 <= 31 as i32 {
        hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, me2,
                                           31 as i32 as usize);
    }
    if 58 as i32 <= 32 as i32 {
        hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, me2,
                                           32 as i32 as usize);
    }
    if 58 as i32 <= 33 as i32 {
        hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, me2,
                                           33 as i32 as usize);
    }
    if 58 as i32 <= 34 as i32 {
        hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, me2,
                                           34 as i32 as usize);
    }
    if 58 as i32 <= 35 as i32 {
        hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, me2,
                                           35 as i32 as usize);
    }
    if 58 as i32 <= 36 as i32 {
        hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, me2,
                                           36 as i32 as usize);
    }
    if 58 as i32 <= 37 as i32 {
        hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, me2,
                                           37 as i32 as usize);
    }
    if 58 as i32 <= 38 as i32 {
        hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, me2,
                                           38 as i32 as usize);
    }
    if 58 as i32 <= 39 as i32 {
        hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, me2,
                                           39 as i32 as usize);
    }
    if 58 as i32 <= 40 as i32 {
        hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, me2,
                                           40 as i32 as usize);
    }
    if 58 as i32 <= 41 as i32 {
        hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, me2,
                                           41 as i32 as usize);
    }
    if 58 as i32 <= 42 as i32 {
        hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, me2,
                                           42 as i32 as usize);
    }
    if 58 as i32 <= 43 as i32 {
        hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, me2,
                                           43 as i32 as usize);
    }
    if 58 as i32 <= 44 as i32 {
        hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, me2,
                                           44 as i32 as usize);
    }
    if 58 as i32 <= 45 as i32 {
        hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, me2,
                                           45 as i32 as usize);
    }
    if 58 as i32 <= 46 as i32 {
        hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, me2,
                                           46 as i32 as usize);
    }
    if 58 as i32 <= 47 as i32 {
        hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, me2,
                                           47 as i32 as usize);
    }
    if 58 as i32 <= 48 as i32 {
        hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, me2,
                                           48 as i32 as usize);
    }
    if 58 as i32 <= 49 as i32 {
        hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, me2,
                                           49 as i32 as usize);
    }
    if 58 as i32 <= 50 as i32 {
        hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, me2,
                                           50 as i32 as usize);
    }
    if 58 as i32 <= 51 as i32 {
        hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, me2,
                                           51 as i32 as usize);
    }
    if 58 as i32 <= 52 as i32 {
        hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, me2,
                                           52 as i32 as usize);
    }
    if 58 as i32 <= 53 as i32 {
        hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, me2,
                                           53 as i32 as usize);
    }
    if 58 as i32 <= 54 as i32 {
        hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, me2,
                                           54 as i32 as usize);
    }
    if 58 as i32 <= 55 as i32 {
        hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, me2,
                                           55 as i32 as usize);
    }
    if 58 as i32 <= 56 as i32 {
        hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, me2,
                                           56 as i32 as usize);
    }
    if 58 as i32 <= 57 as i32 {
        hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, me2,
                                           57 as i32 as usize);
    }
    if 58 as i32 <= 58 as i32 {
        hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, me2,
                                           58 as i32 as usize);
    }
    if 58 as i32 <= 59 as i32 {
        hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, me2,
                                           59 as i32 as usize);
    }
    if 58 as i32 <= 60 as i32 {
        hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, me2,
                                           60 as i32 as usize);
    }
    if 58 as i32 <= 61 as i32 {
        hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, me2,
                                           61 as i32 as usize);
    }
    if 58 as i32 <= 62 as i32 {
        hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, me2,
                                           62 as i32 as usize);
    }
    if 58 as i32 <= 63 as i32 {
        hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, me2,
                                           63 as i32 as usize);
    }
    if 58 as i32 <= 64 as i32 {
        hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, me2,
                                           64 as i32 as usize);
    }
    if 58 as i32 <= 65 as i32 {
        hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, me2,
                                           65 as i32 as usize);
    }
    if 58 as i32 <= 66 as i32 {
        hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, me2,
                                           66 as i32 as usize);
    }
    if 58 as i32 <= 67 as i32 {
        hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, me2,
                                           67 as i32 as usize);
    }
    if 58 as i32 <= 68 as i32 {
        hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, me2,
                                           68 as i32 as usize);
    }
    if 58 as i32 <= 69 as i32 {
        hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, me2,
                                           69 as i32 as usize);
    }
    if 58 as i32 <= 70 as i32 {
        hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, me2,
                                           70 as i32 as usize);
    }
    if 58 as i32 <= 71 as i32 {
        hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, me2,
                                           71 as i32 as usize);
    }
    if 58 as i32 <= 72 as i32 {
        hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, me2,
                                           72 as i32 as usize);
    }
    if 58 as i32 <= 73 as i32 {
        hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, me2,
                                           73 as i32 as usize);
    }
    if 58 as i32 <= 74 as i32 {
        hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, me2,
                                           74 as i32 as usize);
    }
    if 58 as i32 <= 75 as i32 {
        hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, me2,
                                           75 as i32 as usize);
    }
    if 58 as i32 <= 76 as i32 {
        hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, me2,
                                           76 as i32 as usize);
    }
    if 58 as i32 <= 77 as i32 {
        hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, me2,
                                           77 as i32 as usize);
    }
    if 58 as i32 <= 78 as i32 {
        hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, me2,
                                           78 as i32 as usize);
    }
    if 58 as i32 <= 79 as i32 {
        hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, me2,
                                           79 as i32 as usize);
    }
    *ihvout.offset(0 as i32 as isize) =
        (*ihvin.offset(0 as i32 as isize)).wrapping_add(a);
    *ihvout.offset(1 as i32 as isize) =
        (*ihvin.offset(1 as i32 as isize)).wrapping_add(b);
    *ihvout.offset(2 as i32 as isize) =
        (*ihvin.offset(2 as i32 as isize)).wrapping_add(c);
    *ihvout.offset(3 as i32 as isize) =
        (*ihvin.offset(3 as i32 as isize)).wrapping_add(d);
    *ihvout.offset(4 as i32 as isize) =
        (*ihvin.offset(4 as i32 as isize)).wrapping_add(e);
}
unsafe extern "C" fn sha1recompress_fast_65(mut ihvin: *mut uint32_t,
                                            mut ihvout: *mut uint32_t,
                                            mut me2: *const uint32_t,
                                            mut state: *const uint32_t) {
    let mut a: uint32_t = *state.offset(0 as i32 as isize);
    let mut b: uint32_t = *state.offset(1 as i32 as isize);
    let mut c: uint32_t = *state.offset(2 as i32 as isize);
    let mut d: uint32_t = *state.offset(3 as i32 as isize);
    let mut e: uint32_t = *state.offset(4 as i32 as isize);
    if 65 as i32 > 79 as i32 {
        hashclash_sha1compress_round4_step_bw(b, &mut c, d, e, &mut a, me2,
                                              79 as i32 as usize);
    }
    if 65 as i32 > 78 as i32 {
        hashclash_sha1compress_round4_step_bw(c, &mut d, e, a, &mut b, me2,
                                              78 as i32 as usize);
    }
    if 65 as i32 > 77 as i32 {
        hashclash_sha1compress_round4_step_bw(d, &mut e, a, b, &mut c, me2,
                                              77 as i32 as usize);
    }
    if 65 as i32 > 76 as i32 {
        hashclash_sha1compress_round4_step_bw(e, &mut a, b, c, &mut d, me2,
                                              76 as i32 as usize);
    }
    if 65 as i32 > 75 as i32 {
        hashclash_sha1compress_round4_step_bw(a, &mut b, c, d, &mut e, me2,
                                              75 as i32 as usize);
    }
    if 65 as i32 > 74 as i32 {
        hashclash_sha1compress_round4_step_bw(b, &mut c, d, e, &mut a, me2,
                                              74 as i32 as usize);
    }
    if 65 as i32 > 73 as i32 {
        hashclash_sha1compress_round4_step_bw(c, &mut d, e, a, &mut b, me2,
                                              73 as i32 as usize);
    }
    if 65 as i32 > 72 as i32 {
        hashclash_sha1compress_round4_step_bw(d, &mut e, a, b, &mut c, me2,
                                              72 as i32 as usize);
    }
    if 65 as i32 > 71 as i32 {
        hashclash_sha1compress_round4_step_bw(e, &mut a, b, c, &mut d, me2,
                                              71 as i32 as usize);
    }
    if 65 as i32 > 70 as i32 {
        hashclash_sha1compress_round4_step_bw(a, &mut b, c, d, &mut e, me2,
                                              70 as i32 as usize);
    }
    if 65 as i32 > 69 as i32 {
        hashclash_sha1compress_round4_step_bw(b, &mut c, d, e, &mut a, me2,
                                              69 as i32 as usize);
    }
    if 65 as i32 > 68 as i32 {
        hashclash_sha1compress_round4_step_bw(c, &mut d, e, a, &mut b, me2,
                                              68 as i32 as usize);
    }
    if 65 as i32 > 67 as i32 {
        hashclash_sha1compress_round4_step_bw(d, &mut e, a, b, &mut c, me2,
                                              67 as i32 as usize);
    }
    if 65 as i32 > 66 as i32 {
        hashclash_sha1compress_round4_step_bw(e, &mut a, b, c, &mut d, me2,
                                              66 as i32 as usize);
    }
    if 65 as i32 > 65 as i32 {
        hashclash_sha1compress_round4_step_bw(a, &mut b, c, d, &mut e, me2,
                                              65 as i32 as usize);
    }
    if 65 as i32 > 64 as i32 {
        hashclash_sha1compress_round4_step_bw(b, &mut c, d, e, &mut a, me2,
                                              64 as i32 as usize);
    }
    if 65 as i32 > 63 as i32 {
        hashclash_sha1compress_round4_step_bw(c, &mut d, e, a, &mut b, me2,
                                              63 as i32 as usize);
    }
    if 65 as i32 > 62 as i32 {
        hashclash_sha1compress_round4_step_bw(d, &mut e, a, b, &mut c, me2,
                                              62 as i32 as usize);
    }
    if 65 as i32 > 61 as i32 {
        hashclash_sha1compress_round4_step_bw(e, &mut a, b, c, &mut d, me2,
                                              61 as i32 as usize);
    }
    if 65 as i32 > 60 as i32 {
        hashclash_sha1compress_round4_step_bw(a, &mut b, c, d, &mut e, me2,
                                              60 as i32 as usize);
    }
    if 65 as i32 > 59 as i32 {
        hashclash_sha1compress_round3_step_bw(b, &mut c, d, e, &mut a, me2,
                                              59 as i32 as usize);
    }
    if 65 as i32 > 58 as i32 {
        hashclash_sha1compress_round3_step_bw(c, &mut d, e, a, &mut b, me2,
                                              58 as i32 as usize);
    }
    if 65 as i32 > 57 as i32 {
        hashclash_sha1compress_round3_step_bw(d, &mut e, a, b, &mut c, me2,
                                              57 as i32 as usize);
    }
    if 65 as i32 > 56 as i32 {
        hashclash_sha1compress_round3_step_bw(e, &mut a, b, c, &mut d, me2,
                                              56 as i32 as usize);
    }
    if 65 as i32 > 55 as i32 {
        hashclash_sha1compress_round3_step_bw(a, &mut b, c, d, &mut e, me2,
                                              55 as i32 as usize);
    }
    if 65 as i32 > 54 as i32 {
        hashclash_sha1compress_round3_step_bw(b, &mut c, d, e, &mut a, me2,
                                              54 as i32 as usize);
    }
    if 65 as i32 > 53 as i32 {
        hashclash_sha1compress_round3_step_bw(c, &mut d, e, a, &mut b, me2,
                                              53 as i32 as usize);
    }
    if 65 as i32 > 52 as i32 {
        hashclash_sha1compress_round3_step_bw(d, &mut e, a, b, &mut c, me2,
                                              52 as i32 as usize);
    }
    if 65 as i32 > 51 as i32 {
        hashclash_sha1compress_round3_step_bw(e, &mut a, b, c, &mut d, me2,
                                              51 as i32 as usize);
    }
    if 65 as i32 > 50 as i32 {
        hashclash_sha1compress_round3_step_bw(a, &mut b, c, d, &mut e, me2,
                                              50 as i32 as usize);
    }
    if 65 as i32 > 49 as i32 {
        hashclash_sha1compress_round3_step_bw(b, &mut c, d, e, &mut a, me2,
                                              49 as i32 as usize);
    }
    if 65 as i32 > 48 as i32 {
        hashclash_sha1compress_round3_step_bw(c, &mut d, e, a, &mut b, me2,
                                              48 as i32 as usize);
    }
    if 65 as i32 > 47 as i32 {
        hashclash_sha1compress_round3_step_bw(d, &mut e, a, b, &mut c, me2,
                                              47 as i32 as usize);
    }
    if 65 as i32 > 46 as i32 {
        hashclash_sha1compress_round3_step_bw(e, &mut a, b, c, &mut d, me2,
                                              46 as i32 as usize);
    }
    if 65 as i32 > 45 as i32 {
        hashclash_sha1compress_round3_step_bw(a, &mut b, c, d, &mut e, me2,
                                              45 as i32 as usize);
    }
    if 65 as i32 > 44 as i32 {
        hashclash_sha1compress_round3_step_bw(b, &mut c, d, e, &mut a, me2,
                                              44 as i32 as usize);
    }
    if 65 as i32 > 43 as i32 {
        hashclash_sha1compress_round3_step_bw(c, &mut d, e, a, &mut b, me2,
                                              43 as i32 as usize);
    }
    if 65 as i32 > 42 as i32 {
        hashclash_sha1compress_round3_step_bw(d, &mut e, a, b, &mut c, me2,
                                              42 as i32 as usize);
    }
    if 65 as i32 > 41 as i32 {
        hashclash_sha1compress_round3_step_bw(e, &mut a, b, c, &mut d, me2,
                                              41 as i32 as usize);
    }
    if 65 as i32 > 40 as i32 {
        hashclash_sha1compress_round3_step_bw(a, &mut b, c, d, &mut e, me2,
                                              40 as i32 as usize);
    }
    if 65 as i32 > 39 as i32 {
        hashclash_sha1compress_round2_step_bw(b, &mut c, d, e, &mut a, me2,
                                              39 as i32 as usize);
    }
    if 65 as i32 > 38 as i32 {
        hashclash_sha1compress_round2_step_bw(c, &mut d, e, a, &mut b, me2,
                                              38 as i32 as usize);
    }
    if 65 as i32 > 37 as i32 {
        hashclash_sha1compress_round2_step_bw(d, &mut e, a, b, &mut c, me2,
                                              37 as i32 as usize);
    }
    if 65 as i32 > 36 as i32 {
        hashclash_sha1compress_round2_step_bw(e, &mut a, b, c, &mut d, me2,
                                              36 as i32 as usize);
    }
    if 65 as i32 > 35 as i32 {
        hashclash_sha1compress_round2_step_bw(a, &mut b, c, d, &mut e, me2,
                                              35 as i32 as usize);
    }
    if 65 as i32 > 34 as i32 {
        hashclash_sha1compress_round2_step_bw(b, &mut c, d, e, &mut a, me2,
                                              34 as i32 as usize);
    }
    if 65 as i32 > 33 as i32 {
        hashclash_sha1compress_round2_step_bw(c, &mut d, e, a, &mut b, me2,
                                              33 as i32 as usize);
    }
    if 65 as i32 > 32 as i32 {
        hashclash_sha1compress_round2_step_bw(d, &mut e, a, b, &mut c, me2,
                                              32 as i32 as usize);
    }
    if 65 as i32 > 31 as i32 {
        hashclash_sha1compress_round2_step_bw(e, &mut a, b, c, &mut d, me2,
                                              31 as i32 as usize);
    }
    if 65 as i32 > 30 as i32 {
        hashclash_sha1compress_round2_step_bw(a, &mut b, c, d, &mut e, me2,
                                              30 as i32 as usize);
    }
    if 65 as i32 > 29 as i32 {
        hashclash_sha1compress_round2_step_bw(b, &mut c, d, e, &mut a, me2,
                                              29 as i32 as usize);
    }
    if 65 as i32 > 28 as i32 {
        hashclash_sha1compress_round2_step_bw(c, &mut d, e, a, &mut b, me2,
                                              28 as i32 as usize);
    }
    if 65 as i32 > 27 as i32 {
        hashclash_sha1compress_round2_step_bw(d, &mut e, a, b, &mut c, me2,
                                              27 as i32 as usize);
    }
    if 65 as i32 > 26 as i32 {
        hashclash_sha1compress_round2_step_bw(e, &mut a, b, c, &mut d, me2,
                                              26 as i32 as usize);
    }
    if 65 as i32 > 25 as i32 {
        hashclash_sha1compress_round2_step_bw(a, &mut b, c, d, &mut e, me2,
                                              25 as i32 as usize);
    }
    if 65 as i32 > 24 as i32 {
        hashclash_sha1compress_round2_step_bw(b, &mut c, d, e, &mut a, me2,
                                              24 as i32 as usize);
    }
    if 65 as i32 > 23 as i32 {
        hashclash_sha1compress_round2_step_bw(c, &mut d, e, a, &mut b, me2,
                                              23 as i32 as usize);
    }
    if 65 as i32 > 22 as i32 {
        hashclash_sha1compress_round2_step_bw(d, &mut e, a, b, &mut c, me2,
                                              22 as i32 as usize);
    }
    if 65 as i32 > 21 as i32 {
        hashclash_sha1compress_round2_step_bw(e, &mut a, b, c, &mut d, me2,
                                              21 as i32 as usize);
    }
    if 65 as i32 > 20 as i32 {
        hashclash_sha1compress_round2_step_bw(a, &mut b, c, d, &mut e, me2,
                                              20 as i32 as usize);
    }
    if 65 as i32 > 19 as i32 {
        hashclash_sha1compress_round1_step_bw(b, &mut c, d, e, &mut a, me2,
                                              19 as i32 as usize);
    }
    if 65 as i32 > 18 as i32 {
        hashclash_sha1compress_round1_step_bw(c, &mut d, e, a, &mut b, me2,
                                              18 as i32 as usize);
    }
    if 65 as i32 > 17 as i32 {
        hashclash_sha1compress_round1_step_bw(d, &mut e, a, b, &mut c, me2,
                                              17 as i32 as usize);
    }
    if 65 as i32 > 16 as i32 {
        hashclash_sha1compress_round1_step_bw(e, &mut a, b, c, &mut d, me2,
                                              16 as i32 as usize);
    }
    if 65 as i32 > 15 as i32 {
        hashclash_sha1compress_round1_step_bw(a, &mut b, c, d, &mut e, me2,
                                              15 as i32 as usize);
    }
    if 65 as i32 > 14 as i32 {
        hashclash_sha1compress_round1_step_bw(b, &mut c, d, e, &mut a, me2,
                                              14 as i32 as usize);
    }
    if 65 as i32 > 13 as i32 {
        hashclash_sha1compress_round1_step_bw(c, &mut d, e, a, &mut b, me2,
                                              13 as i32 as usize);
    }
    if 65 as i32 > 12 as i32 {
        hashclash_sha1compress_round1_step_bw(d, &mut e, a, b, &mut c, me2,
                                              12 as i32 as usize);
    }
    if 65 as i32 > 11 as i32 {
        hashclash_sha1compress_round1_step_bw(e, &mut a, b, c, &mut d, me2,
                                              11 as i32 as usize);
    }
    if 65 as i32 > 10 as i32 {
        hashclash_sha1compress_round1_step_bw(a, &mut b, c, d, &mut e, me2,
                                              10 as i32 as usize);
    }
    if 65 as i32 > 9 as i32 {
        hashclash_sha1compress_round1_step_bw(b, &mut c, d, e, &mut a, me2,
                                              9 as i32 as usize);
    }
    if 65 as i32 > 8 as i32 {
        hashclash_sha1compress_round1_step_bw(c, &mut d, e, a, &mut b, me2,
                                              8 as i32 as usize);
    }
    if 65 as i32 > 7 as i32 {
        hashclash_sha1compress_round1_step_bw(d, &mut e, a, b, &mut c, me2,
                                              7 as i32 as usize);
    }
    if 65 as i32 > 6 as i32 {
        hashclash_sha1compress_round1_step_bw(e, &mut a, b, c, &mut d, me2,
                                              6 as i32 as usize);
    }
    if 65 as i32 > 5 as i32 {
        hashclash_sha1compress_round1_step_bw(a, &mut b, c, d, &mut e, me2,
                                              5 as i32 as usize);
    }
    if 65 as i32 > 4 as i32 {
        hashclash_sha1compress_round1_step_bw(b, &mut c, d, e, &mut a, me2,
                                              4 as i32 as usize);
    }
    if 65 as i32 > 3 as i32 {
        hashclash_sha1compress_round1_step_bw(c, &mut d, e, a, &mut b, me2,
                                              3 as i32 as usize);
    }
    if 65 as i32 > 2 as i32 {
        hashclash_sha1compress_round1_step_bw(d, &mut e, a, b, &mut c, me2,
                                              2 as i32 as usize);
    }
    if 65 as i32 > 1 as i32 {
        hashclash_sha1compress_round1_step_bw(e, &mut a, b, c, &mut d, me2,
                                              1 as i32 as usize);
    }
    if 65 as i32 > 0 as i32 {
        hashclash_sha1compress_round1_step_bw(a, &mut b, c, d, &mut e, me2,
                                              0 as i32 as usize);
    }
    *ihvin.offset(0 as i32 as isize) = a;
    *ihvin.offset(1 as i32 as isize) = b;
    *ihvin.offset(2 as i32 as isize) = c;
    *ihvin.offset(3 as i32 as isize) = d;
    *ihvin.offset(4 as i32 as isize) = e;
    a = *state.offset(0 as i32 as isize);
    b = *state.offset(1 as i32 as isize);
    c = *state.offset(2 as i32 as isize);
    d = *state.offset(3 as i32 as isize);
    e = *state.offset(4 as i32 as isize);
    if 65 as i32 <= 0 as i32 {
        hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, me2,
                                           0 as i32 as usize);
    }
    if 65 as i32 <= 1 as i32 {
        hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, me2,
                                           1 as i32 as usize);
    }
    if 65 as i32 <= 2 as i32 {
        hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, me2,
                                           2 as i32 as usize);
    }
    if 65 as i32 <= 3 as i32 {
        hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, me2,
                                           3 as i32 as usize);
    }
    if 65 as i32 <= 4 as i32 {
        hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, me2,
                                           4 as i32 as usize);
    }
    if 65 as i32 <= 5 as i32 {
        hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, me2,
                                           5 as i32 as usize);
    }
    if 65 as i32 <= 6 as i32 {
        hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, me2,
                                           6 as i32 as usize);
    }
    if 65 as i32 <= 7 as i32 {
        hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, me2,
                                           7 as i32 as usize);
    }
    if 65 as i32 <= 8 as i32 {
        hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, me2,
                                           8 as i32 as usize);
    }
    if 65 as i32 <= 9 as i32 {
        hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, me2,
                                           9 as i32 as usize);
    }
    if 65 as i32 <= 10 as i32 {
        hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, me2,
                                           10 as i32 as usize);
    }
    if 65 as i32 <= 11 as i32 {
        hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, me2,
                                           11 as i32 as usize);
    }
    if 65 as i32 <= 12 as i32 {
        hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, me2,
                                           12 as i32 as usize);
    }
    if 65 as i32 <= 13 as i32 {
        hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, me2,
                                           13 as i32 as usize);
    }
    if 65 as i32 <= 14 as i32 {
        hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, me2,
                                           14 as i32 as usize);
    }
    if 65 as i32 <= 15 as i32 {
        hashclash_sha1compress_round1_step(a, &mut b, c, d, &mut e, me2,
                                           15 as i32 as usize);
    }
    if 65 as i32 <= 16 as i32 {
        hashclash_sha1compress_round1_step(e, &mut a, b, c, &mut d, me2,
                                           16 as i32 as usize);
    }
    if 65 as i32 <= 17 as i32 {
        hashclash_sha1compress_round1_step(d, &mut e, a, b, &mut c, me2,
                                           17 as i32 as usize);
    }
    if 65 as i32 <= 18 as i32 {
        hashclash_sha1compress_round1_step(c, &mut d, e, a, &mut b, me2,
                                           18 as i32 as usize);
    }
    if 65 as i32 <= 19 as i32 {
        hashclash_sha1compress_round1_step(b, &mut c, d, e, &mut a, me2,
                                           19 as i32 as usize);
    }
    if 65 as i32 <= 20 as i32 {
        hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, me2,
                                           20 as i32 as usize);
    }
    if 65 as i32 <= 21 as i32 {
        hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, me2,
                                           21 as i32 as usize);
    }
    if 65 as i32 <= 22 as i32 {
        hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, me2,
                                           22 as i32 as usize);
    }
    if 65 as i32 <= 23 as i32 {
        hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, me2,
                                           23 as i32 as usize);
    }
    if 65 as i32 <= 24 as i32 {
        hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, me2,
                                           24 as i32 as usize);
    }
    if 65 as i32 <= 25 as i32 {
        hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, me2,
                                           25 as i32 as usize);
    }
    if 65 as i32 <= 26 as i32 {
        hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, me2,
                                           26 as i32 as usize);
    }
    if 65 as i32 <= 27 as i32 {
        hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, me2,
                                           27 as i32 as usize);
    }
    if 65 as i32 <= 28 as i32 {
        hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, me2,
                                           28 as i32 as usize);
    }
    if 65 as i32 <= 29 as i32 {
        hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, me2,
                                           29 as i32 as usize);
    }
    if 65 as i32 <= 30 as i32 {
        hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, me2,
                                           30 as i32 as usize);
    }
    if 65 as i32 <= 31 as i32 {
        hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, me2,
                                           31 as i32 as usize);
    }
    if 65 as i32 <= 32 as i32 {
        hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, me2,
                                           32 as i32 as usize);
    }
    if 65 as i32 <= 33 as i32 {
        hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, me2,
                                           33 as i32 as usize);
    }
    if 65 as i32 <= 34 as i32 {
        hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, me2,
                                           34 as i32 as usize);
    }
    if 65 as i32 <= 35 as i32 {
        hashclash_sha1compress_round2_step(a, &mut b, c, d, &mut e, me2,
                                           35 as i32 as usize);
    }
    if 65 as i32 <= 36 as i32 {
        hashclash_sha1compress_round2_step(e, &mut a, b, c, &mut d, me2,
                                           36 as i32 as usize);
    }
    if 65 as i32 <= 37 as i32 {
        hashclash_sha1compress_round2_step(d, &mut e, a, b, &mut c, me2,
                                           37 as i32 as usize);
    }
    if 65 as i32 <= 38 as i32 {
        hashclash_sha1compress_round2_step(c, &mut d, e, a, &mut b, me2,
                                           38 as i32 as usize);
    }
    if 65 as i32 <= 39 as i32 {
        hashclash_sha1compress_round2_step(b, &mut c, d, e, &mut a, me2,
                                           39 as i32 as usize);
    }
    if 65 as i32 <= 40 as i32 {
        hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, me2,
                                           40 as i32 as usize);
    }
    if 65 as i32 <= 41 as i32 {
        hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, me2,
                                           41 as i32 as usize);
    }
    if 65 as i32 <= 42 as i32 {
        hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, me2,
                                           42 as i32 as usize);
    }
    if 65 as i32 <= 43 as i32 {
        hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, me2,
                                           43 as i32 as usize);
    }
    if 65 as i32 <= 44 as i32 {
        hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, me2,
                                           44 as i32 as usize);
    }
    if 65 as i32 <= 45 as i32 {
        hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, me2,
                                           45 as i32 as usize);
    }
    if 65 as i32 <= 46 as i32 {
        hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, me2,
                                           46 as i32 as usize);
    }
    if 65 as i32 <= 47 as i32 {
        hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, me2,
                                           47 as i32 as usize);
    }
    if 65 as i32 <= 48 as i32 {
        hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, me2,
                                           48 as i32 as usize);
    }
    if 65 as i32 <= 49 as i32 {
        hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, me2,
                                           49 as i32 as usize);
    }
    if 65 as i32 <= 50 as i32 {
        hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, me2,
                                           50 as i32 as usize);
    }
    if 65 as i32 <= 51 as i32 {
        hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, me2,
                                           51 as i32 as usize);
    }
    if 65 as i32 <= 52 as i32 {
        hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, me2,
                                           52 as i32 as usize);
    }
    if 65 as i32 <= 53 as i32 {
        hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, me2,
                                           53 as i32 as usize);
    }
    if 65 as i32 <= 54 as i32 {
        hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, me2,
                                           54 as i32 as usize);
    }
    if 65 as i32 <= 55 as i32 {
        hashclash_sha1compress_round3_step(a, &mut b, c, d, &mut e, me2,
                                           55 as i32 as usize);
    }
    if 65 as i32 <= 56 as i32 {
        hashclash_sha1compress_round3_step(e, &mut a, b, c, &mut d, me2,
                                           56 as i32 as usize);
    }
    if 65 as i32 <= 57 as i32 {
        hashclash_sha1compress_round3_step(d, &mut e, a, b, &mut c, me2,
                                           57 as i32 as usize);
    }
    if 65 as i32 <= 58 as i32 {
        hashclash_sha1compress_round3_step(c, &mut d, e, a, &mut b, me2,
                                           58 as i32 as usize);
    }
    if 65 as i32 <= 59 as i32 {
        hashclash_sha1compress_round3_step(b, &mut c, d, e, &mut a, me2,
                                           59 as i32 as usize);
    }
    if 65 as i32 <= 60 as i32 {
        hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, me2,
                                           60 as i32 as usize);
    }
    if 65 as i32 <= 61 as i32 {
        hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, me2,
                                           61 as i32 as usize);
    }
    if 65 as i32 <= 62 as i32 {
        hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, me2,
                                           62 as i32 as usize);
    }
    if 65 as i32 <= 63 as i32 {
        hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, me2,
                                           63 as i32 as usize);
    }
    if 65 as i32 <= 64 as i32 {
        hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, me2,
                                           64 as i32 as usize);
    }
    if 65 as i32 <= 65 as i32 {
        hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, me2,
                                           65 as i32 as usize);
    }
    if 65 as i32 <= 66 as i32 {
        hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, me2,
                                           66 as i32 as usize);
    }
    if 65 as i32 <= 67 as i32 {
        hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, me2,
                                           67 as i32 as usize);
    }
    if 65 as i32 <= 68 as i32 {
        hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, me2,
                                           68 as i32 as usize);
    }
    if 65 as i32 <= 69 as i32 {
        hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, me2,
                                           69 as i32 as usize);
    }
    if 65 as i32 <= 70 as i32 {
        hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, me2,
                                           70 as i32 as usize);
    }
    if 65 as i32 <= 71 as i32 {
        hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, me2,
                                           71 as i32 as usize);
    }
    if 65 as i32 <= 72 as i32 {
        hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, me2,
                                           72 as i32 as usize);
    }
    if 65 as i32 <= 73 as i32 {
        hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, me2,
                                           73 as i32 as usize);
    }
    if 65 as i32 <= 74 as i32 {
        hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, me2,
                                           74 as i32 as usize);
    }
    if 65 as i32 <= 75 as i32 {
        hashclash_sha1compress_round4_step(a, &mut b, c, d, &mut e, me2,
                                           75 as i32 as usize);
    }
    if 65 as i32 <= 76 as i32 {
        hashclash_sha1compress_round4_step(e, &mut a, b, c, &mut d, me2,
                                           76 as i32 as usize);
    }
    if 65 as i32 <= 77 as i32 {
        hashclash_sha1compress_round4_step(d, &mut e, a, b, &mut c, me2,
                                           77 as i32 as usize);
    }
    if 65 as i32 <= 78 as i32 {
        hashclash_sha1compress_round4_step(c, &mut d, e, a, &mut b, me2,
                                           78 as i32 as usize);
    }
    if 65 as i32 <= 79 as i32 {
        hashclash_sha1compress_round4_step(b, &mut c, d, e, &mut a, me2,
                                           79 as i32 as usize);
    }
    *ihvout.offset(0 as i32 as isize) =
        (*ihvin.offset(0 as i32 as isize)).wrapping_add(a);
    *ihvout.offset(1 as i32 as isize) =
        (*ihvin.offset(1 as i32 as isize)).wrapping_add(b);
    *ihvout.offset(2 as i32 as isize) =
        (*ihvin.offset(2 as i32 as isize)).wrapping_add(c);
    *ihvout.offset(3 as i32 as isize) =
        (*ihvin.offset(3 as i32 as isize)).wrapping_add(d);
    *ihvout.offset(4 as i32 as isize) =
        (*ihvin.offset(4 as i32 as isize)).wrapping_add(e);
}
unsafe extern "C" fn sha1_recompression_step(mut step: uint32_t,
                                             mut ihvin: *mut uint32_t,
                                             mut ihvout: *mut uint32_t,
                                             mut me2: *const uint32_t,
                                             mut state: *const uint32_t) {
    match step {
        58 => { sha1recompress_fast_58(ihvin, ihvout, me2, state); }
        65 => { sha1recompress_fast_65(ihvin, ihvout, me2, state); }
        _ => { panic!(); }
    };
}
/*
   Because Little-Endian architectures are most common,
   we only set SHA1DC_BIGENDIAN if one of these conditions is met.
   Note that all MSFT platforms are little endian,
   so none of these will be defined under the MSC compiler.
   If you are compiling on a big endian platform and your compiler does not define one of these,
   you will have to add whatever macros your tool chain defines to indicate Big-Endianness.
 */
/*
 * Should detect Big Endian under GCC since at least 4.6.0 (gcc svn
 * rev #165881). See
 * https://gcc.gnu.org/onlinedocs/cpp/Common-Predefined-Macros.html
 *
 * This also works under clang since 3.2, it copied the GCC-ism. See
 * clang.git's 3b198a97d2 ("Preprocessor: add __BYTE_ORDER__
 * predefined macro", 2012-07-27)
 */
/* Not under GCC-alike */
/* Big Endian detection */
/*ENDIANNESS SELECTION*/
/*UNALIGNED ACCESS DETECTION*/
/*FORCE ALIGNED ACCESS*/
unsafe extern "C" fn sha1_process(mut ctx: *mut SHA1_CTX,
                                  mut block: *const uint32_t) {
    let mut i: u32 = 0;
    let mut j: u32 = 0;
    let mut ubc_dv_mask: [uint32_t; 1] = [0xffffffff as u32];
    let mut ihvtmp: [uint32_t; 5] = [0; 5];
    (*ctx).ihv1[0 as i32 as usize] =
        (*ctx).ihv[0 as i32 as usize];
    (*ctx).ihv1[1 as i32 as usize] =
        (*ctx).ihv[1 as i32 as usize];
    (*ctx).ihv1[2 as i32 as usize] =
        (*ctx).ihv[2 as i32 as usize];
    (*ctx).ihv1[3 as i32 as usize] =
        (*ctx).ihv[3 as i32 as usize];
    (*ctx).ihv1[4 as i32 as usize] =
        (*ctx).ihv[4 as i32 as usize];
    sha1_compression_states((*ctx).ihv.as_mut_ptr(), block,
                            (*ctx).m1.as_mut_ptr(),
                            (*ctx).states.as_mut_ptr());
    if (*ctx).detect_coll {
        if (*ctx).ubc_check {
            ubc_check((*ctx).m1.as_mut_ptr() as *const uint32_t,
                      ubc_dv_mask.as_mut_ptr());
        }
        if ubc_dv_mask[0 as i32 as usize] !=
               0 as i32 as u32 {
            i = 0 as i32 as u32;
            while (*sha1_dvs.as_mut_ptr().offset(i as isize)).dvType !=
                      0 as i32 {
                if ubc_dv_mask[0 as i32 as usize] &
                       (1 as i32 as uint32_t) <<
                           (*sha1_dvs.as_mut_ptr().offset(i as isize)).maskb
                       != 0 {
                    j = 0 as i32 as u32;
                    while j < 80 as i32 as u32 {
                        (*ctx).m2[j as usize] =
                            (*ctx).m1[j as usize] ^
                                (*sha1_dvs.as_mut_ptr().offset(i as
                                                                   isize)).dm[j
                                                                                  as
                                                                                  usize];
                        j = j.wrapping_add(1)
                    }
                    sha1_recompression_step((*sha1_dvs.as_mut_ptr().offset(i
                                                                               as
                                                                               isize)).testt
                                                as uint32_t,
                                            (*ctx).ihv2.as_mut_ptr(),
                                            ihvtmp.as_mut_ptr(),
                                            (*ctx).m2.as_mut_ptr() as
                                                *const uint32_t,
                                            (*ctx).states[(*sha1_dvs.as_mut_ptr().offset(i
                                                                                             as
                                                                                             isize)).testt
                                                              as
                                                              usize].as_mut_ptr()
                                                as *const uint32_t);
                    /* to verify SHA-1 collision detection code with collisions for reduced-step SHA-1 */
                    if 0 as i32 as u32 ==
                           ihvtmp[0 as i32 as usize] ^
                               (*ctx).ihv[0 as i32 as usize] |
                               ihvtmp[1 as i32 as usize] ^
                                   (*ctx).ihv[1 as i32 as usize] |
                               ihvtmp[2 as i32 as usize] ^
                                   (*ctx).ihv[2 as i32 as usize] |
                               ihvtmp[3 as i32 as usize] ^
                                   (*ctx).ihv[3 as i32 as usize] |
                               ihvtmp[4 as i32 as usize] ^
                                   (*ctx).ihv[4 as i32 as usize] ||
                           (*ctx).reduced_round_coll &&
                               0 as i32 as u32 ==
                                   (*ctx).ihv1[0 as i32 as usize] ^
                                       (*ctx).ihv2[0 as i32 as usize]
                                       |
                                       (*ctx).ihv1[1 as i32 as usize]
                                           ^
                                           (*ctx).ihv2[1 as i32 as
                                                           usize] |
                                       (*ctx).ihv1[2 as i32 as usize]
                                           ^
                                           (*ctx).ihv2[2 as i32 as
                                                           usize] |
                                       (*ctx).ihv1[3 as i32 as usize]
                                           ^
                                           (*ctx).ihv2[3 as i32 as
                                                           usize] |
                                       (*ctx).ihv1[4 as i32 as usize]
                                           ^
                                           (*ctx).ihv2[4 as i32 as
                                                           usize] {
                        (*ctx).found_collision = true;
                        if (*ctx).safe_hash {
                            sha1_compression_W((*ctx).ihv.as_mut_ptr(),
                                               (*ctx).m1.as_mut_ptr() as
                                                   *const uint32_t);
                            sha1_compression_W((*ctx).ihv.as_mut_ptr(),
                                               (*ctx).m1.as_mut_ptr() as
                                                   *const uint32_t);
                        }
                        break ;
                    }
                }
                i = i.wrapping_add(1)
            }
        }
    };
}
pub unsafe fn SHA1DCInit(mut ctx: *mut SHA1_CTX) {
    (*ctx).total = 0 as i32 as uint64_t;
    (*ctx).ihv[0 as i32 as usize] =
        0x67452301 as i32 as uint32_t;
    (*ctx).ihv[1 as i32 as usize] = 0xefcdab89 as u32;
    (*ctx).ihv[2 as i32 as usize] = 0x98badcfe as u32;
    (*ctx).ihv[3 as i32 as usize] =
        0x10325476 as i32 as uint32_t;
    (*ctx).ihv[4 as i32 as usize] = 0xc3d2e1f0 as u32;
    (*ctx).found_collision = false;
    (*ctx).safe_hash = true;
    (*ctx).ubc_check = true;
    (*ctx).detect_coll = true;
    (*ctx).reduced_round_coll = false;
    (*ctx).callback = None;
}
/* **
* Copyright 2017 Marc Stevens <marc@marc-stevens.nl>, Dan Shumow <danshu@microsoft.com>
* Distributed under the MIT Software License.
* See accompanying file LICENSE.txt or copy at
* https://opensource.org/licenses/MIT
***/
/* sha-1 compression function that takes an already expanded message, and additionally store intermediate states */
/* only stores states ii (the state between step ii-1 and step ii) when DOSTORESTATEii is defined in ubc_check.h */
/*
// Function type for sha1_recompression_step_T (uint32_t ihvin[5], uint32_t ihvout[5], const uint32_t me2[80], const uint32_t state[5]).
// Where 0 <= T < 80
//       me2 is an expanded message (the expansion of an original message block XOR'ed with a disturbance vector's message block difference.)
//       state is the internal state (a,b,c,d,e) before step T of the SHA-1 compression function while processing the original message block.
// The function will return:
//       ihvin: The reconstructed input chaining value.
//       ihvout: The reconstructed output chaining value.
*/
/* A callback function type that can be set to be called when a collision block has been found: */
/* void collision_block_callback(uint64_t byteoffset, const uint32_t ihvin1[5], const uint32_t ihvin2[5], const uint32_t m1[80], const uint32_t m2[80]) */
/* The SHA-1 context. */
/* Initialize SHA-1 context. */
/*
    Function to enable safe SHA-1 hashing:
    Collision attacks are thwarted by hashing a detected near-collision block 3 times.
    Think of it as extending SHA-1 from 80-steps to 240-steps for such blocks:
        The best collision attacks against SHA-1 have complexity about 2^60,
        thus for 240-steps an immediate lower-bound for the best cryptanalytic attacks would be 2^180.
        An attacker would be better off using a generic birthday search of complexity 2^80.

   Enabling safe SHA-1 hashing will result in the correct SHA-1 hash for messages where no collision attack was detected,
   but it will result in a different SHA-1 hash for messages where a collision attack was detected.
   This will automatically invalidate SHA-1 based digital signature forgeries.
   Enabled by default.
*/
pub unsafe fn SHA1DCSetSafeHash(mut ctx: *mut SHA1_CTX,
                                mut safehash: i32) {
    if safehash != 0 {
        (*ctx).safe_hash = true
    } else { (*ctx).safe_hash = false };
}
/*
    Function to disable or enable the use of Unavoidable Bitconditions (provides a significant speed up).
    Enabled by default
 */
pub unsafe fn SHA1DCSetUseUBC(mut ctx: *mut SHA1_CTX,
                              mut ubc_check_0: i32) {
    if ubc_check_0 != 0 {
        (*ctx).ubc_check = true
    } else { (*ctx).ubc_check = false };
}
/*
    Function to disable or enable the use of Collision Detection.
    Enabled by default.
 */
pub unsafe fn SHA1DCSetUseDetectColl(mut ctx: *mut SHA1_CTX,
                                     mut detect_coll: i32) {
    if detect_coll != 0 {
        (*ctx).detect_coll = true
    } else { (*ctx).detect_coll = false };
}
/* function to disable or enable the detection of reduced-round SHA-1 collisions */
/* disabled by default */
pub unsafe fn SHA1DCSetDetectReducedRoundCollision(mut ctx:
                                                   *mut SHA1_CTX,
                                                   mut reduced_round_coll: i32) {
    if reduced_round_coll != 0 {
        (*ctx).reduced_round_coll = true
    } else { (*ctx).reduced_round_coll = false };
}
/* function to set a callback function, pass NULL to disable */
/* by default no callback set */
pub unsafe fn SHA1DCSetCallback(mut ctx: *mut SHA1_CTX,
                                mut callback: collision_block_callback) {
    (*ctx).callback = callback;
}
/* update SHA-1 context with buffer contents */
pub unsafe fn SHA1DCUpdate(mut ctx: *mut SHA1_CTX,
                           mut buf: *const i8,
                           mut len: usize) {
    let mut left: u32 = 0;
    let mut fill: u32 = 0;
    if len == 0 { return }
    left =
        ((*ctx).total & 63) as u32;
    fill = (64 as i32 as u32).wrapping_sub(left);
    if left != 0 && len >= fill as usize {
        (*ctx).total =
            ((*ctx).total as
                 u64).wrapping_add(fill as u64) as
                uint64_t as uint64_t;
        memcpy((*ctx).buffer.as_mut_ptr().offset(left as isize) as
                   *mut core::ffi::c_void, buf as *const core::ffi::c_void,
               fill as usize);
        sha1_process(ctx,
                     (*ctx).buffer.as_mut_ptr() as *mut uint32_t as
                         *const uint32_t);
        buf = buf.offset(fill as isize);
        len =
            (len as u64).wrapping_sub(fill as u64) as
                usize as usize;
        left = 0 as i32 as u32
    }
    while len >= 64 {
        (*ctx).total =
            ((*ctx).total as
                 u64).wrapping_add(64 as i32 as
                                                 u64) as uint64_t as
                uint64_t;
        sha1_process_unaligned(ctx, buf as *const core::ffi::c_void);
        buf = buf.offset(64 as i32 as isize);
        len =
            (len as
                 u64).wrapping_sub(64 as i32 as
                                                 u64) as usize as
                usize
    }
    if len > 0 {
        (*ctx).total = (*ctx).total.wrapping_add(len as uint64_t);
        memcpy((*ctx).buffer.as_mut_ptr().offset(left as isize) as
                   *mut core::ffi::c_void, buf as *const core::ffi::c_void, len);
    };
}
static mut sha1_padding: [u8; 64] =
    [0x80 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8,
     0 as i32 as u8, 0 as i32 as u8];
/* obtain SHA-1 hash from SHA-1 context */
/* returns: 0 = no collision detected, otherwise = collision found => warn user for active attack */
pub unsafe fn SHA1DCFinal(mut output: *mut u8,
                          mut ctx: *mut SHA1_CTX) -> bool {
    let mut last: uint32_t =
        ((*ctx).total & 63) as uint32_t;
    let mut padn: uint32_t =
        if last < 56 as i32 as u32 {
            (56 as i32 as u32).wrapping_sub(last)
        } else { (120 as i32 as u32).wrapping_sub(last) };
    let mut total: uint64_t = 0;
    SHA1DCUpdate(ctx, sha1_padding.as_ptr() as *const i8,
                 padn as usize);
    total = (*ctx).total.wrapping_sub(padn as uint64_t);
    total <<= 3 as i32;
    (*ctx).buffer[56 as i32 as usize] =
        (total >> 56 as i32) as u8;
    (*ctx).buffer[57 as i32 as usize] =
        (total >> 48 as i32) as u8;
    (*ctx).buffer[58 as i32 as usize] =
        (total >> 40 as i32) as u8;
    (*ctx).buffer[59 as i32 as usize] =
        (total >> 32 as i32) as u8;
    (*ctx).buffer[60 as i32 as usize] =
        (total >> 24 as i32) as u8;
    (*ctx).buffer[61 as i32 as usize] =
        (total >> 16 as i32) as u8;
    (*ctx).buffer[62 as i32 as usize] =
        (total >> 8 as i32) as u8;
    (*ctx).buffer[63 as i32 as usize] = total as u8;
    sha1_process(ctx,
                 (*ctx).buffer.as_mut_ptr() as *mut uint32_t as
                     *const uint32_t);
    *output.offset(0 as i32 as isize) =
        ((*ctx).ihv[0 as i32 as usize] >> 24 as i32) as
            u8;
    *output.offset(1 as i32 as isize) =
        ((*ctx).ihv[0 as i32 as usize] >> 16 as i32) as
            u8;
    *output.offset(2 as i32 as isize) =
        ((*ctx).ihv[0 as i32 as usize] >> 8 as i32) as
            u8;
    *output.offset(3 as i32 as isize) =
        (*ctx).ihv[0 as i32 as usize] as u8;
    *output.offset(4 as i32 as isize) =
        ((*ctx).ihv[1 as i32 as usize] >> 24 as i32) as
            u8;
    *output.offset(5 as i32 as isize) =
        ((*ctx).ihv[1 as i32 as usize] >> 16 as i32) as
            u8;
    *output.offset(6 as i32 as isize) =
        ((*ctx).ihv[1 as i32 as usize] >> 8 as i32) as
            u8;
    *output.offset(7 as i32 as isize) =
        (*ctx).ihv[1 as i32 as usize] as u8;
    *output.offset(8 as i32 as isize) =
        ((*ctx).ihv[2 as i32 as usize] >> 24 as i32) as
            u8;
    *output.offset(9 as i32 as isize) =
        ((*ctx).ihv[2 as i32 as usize] >> 16 as i32) as
            u8;
    *output.offset(10 as i32 as isize) =
        ((*ctx).ihv[2 as i32 as usize] >> 8 as i32) as
            u8;
    *output.offset(11 as i32 as isize) =
        (*ctx).ihv[2 as i32 as usize] as u8;
    *output.offset(12 as i32 as isize) =
        ((*ctx).ihv[3 as i32 as usize] >> 24 as i32) as
            u8;
    *output.offset(13 as i32 as isize) =
        ((*ctx).ihv[3 as i32 as usize] >> 16 as i32) as
            u8;
    *output.offset(14 as i32 as isize) =
        ((*ctx).ihv[3 as i32 as usize] >> 8 as i32) as
            u8;
    *output.offset(15 as i32 as isize) =
        (*ctx).ihv[3 as i32 as usize] as u8;
    *output.offset(16 as i32 as isize) =
        ((*ctx).ihv[4 as i32 as usize] >> 24 as i32) as
            u8;
    *output.offset(17 as i32 as isize) =
        ((*ctx).ihv[4 as i32 as usize] >> 16 as i32) as
            u8;
    *output.offset(18 as i32 as isize) =
        ((*ctx).ihv[4 as i32 as usize] >> 8 as i32) as
            u8;
    *output.offset(19 as i32 as isize) =
        (*ctx).ihv[4 as i32 as usize] as u8;
    return (*ctx).found_collision;
}

#![no_std]

#![cfg_attr(feature = "nightly", feature(core_intrinsics))]
#![allow(clippy::missing_safety_doc)]

mod mlock;
mod alloc;

use core::ptr;

#[cfg(feature = "use_os")]
pub use mlock::{ mlock, munlock };

#[cfg(feature = "alloc")]
pub use alloc::{ Prot, mprotect, malloc, malloc_sized, free };


// -- memcmp --

/// Secure `memeq`.
#[inline(never)]
pub unsafe fn memeq(b1: *const u8, b2: *const u8, len: usize) -> bool {
    (0..len)
        .map(|i| ptr::read_volatile(b1.add(i)) ^ ptr::read_volatile(b2.add(i)))
        .fold(0, |sum, next| sum | next)
        .eq(&0)
}


/// Secure `memcmp`.
#[inline(never)]
pub unsafe fn memcmp(b1: *const u8, b2: *const u8, len: usize) -> i32 {
    let mut res = 0;
    for i in (0..len).rev() {
        let diff = i32::from(ptr::read_volatile(b1.add(i)))
            - i32::from(ptr::read_volatile(b2.add(i)));
        res = (res & (((diff - 1) & !diff) >> 8)) | diff;
    }
    ((res - 1) >> 8) + (res >> 8) + 1
}


// -- memset / memzero --

/// General `memset`.
#[inline(never)]
pub unsafe fn memset(s: *mut u8, c: u8, n: usize) {
    #[cfg(feature = "nightly")] {
        core::intrinsics::volatile_set_memory(s, c, n);
    }

    #[cfg(not(feature = "nightly"))] {
        let s = ptr::read_volatile(&s);
        let c = ptr::read_volatile(&c);
        let n = ptr::read_volatile(&n);

        for i in 0..n {
            ptr::write(s.add(i), c);
        }

        let _ = ptr::read_volatile(&s);
    }
}

/// General `memzero`.
#[inline]
pub unsafe fn memzero(dest: *mut u8, n: usize) {
    memset(dest, 0, n);
}

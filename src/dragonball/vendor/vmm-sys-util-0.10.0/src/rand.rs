// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! Miscellaneous functions related to getting (pseudo) random numbers and
//! strings.
//!
//! NOTE! This should not be used when you do need __real__ random numbers such
//! as for encryption but will probably be suitable when you want locally
//! unique ID's that will not be shared over the network.

use std::ffi::OsString;
use std::str;

/// Gets an ever increasing u64 (at least for this process).
///
/// The number retrieved will be based upon the time of the last reboot (x86_64)
/// and something undefined for other architectures.
pub fn timestamp_cycles() -> u64 {
    #[cfg(target_arch = "x86_64")]
    // Safe because there's nothing that can go wrong with this call.
    unsafe {
        std::arch::x86_64::_rdtsc() as u64
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        const MONOTONIC_CLOCK_MULTPIPLIER: u64 = 1_000_000_000;

        let mut ts = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };

        unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
        }
        (ts.tv_sec as u64) * MONOTONIC_CLOCK_MULTPIPLIER + (ts.tv_nsec as u64)
    }
}

/// Generate pseudo random u32 numbers based on the current timestamp.
pub fn xor_psuedo_rng_u32() -> u32 {
    let mut t: u32 = timestamp_cycles() as u32;
    // Taken from https://en.wikipedia.org/wiki/Xorshift
    t ^= t << 13;
    t ^= t >> 17;
    t ^ (t << 5)
}

// This will get an array of numbers that can safely be converted to strings
// because they will be in the range [a-zA-Z0-9]. The return vector could be any
// size between 0 and 4.
fn xor_psuedo_rng_u8_alphanumerics(rand_fn: &dyn Fn() -> u32) -> Vec<u8> {
    let mut r = vec![];

    fn between(lower: u8, upper: u8, to_check: u8) -> bool {
        (to_check >= lower) && (to_check <= upper)
    }

    for n in &rand_fn().to_ne_bytes() {
        // Upper / Lower alphabetics and numbers.
        if between(48, 57, *n) || between(65, 90, *n) || between(97, 122, *n) {
            r.push(*n);
        }
    }
    r
}

fn rand_alphanumerics_impl(rand_fn: &dyn Fn() -> u32, len: usize) -> OsString {
    let mut buf = OsString::new();
    let mut done = 0;
    loop {
        for n in xor_psuedo_rng_u8_alphanumerics(rand_fn) {
            done += 1;
            buf.push(str::from_utf8(&[n]).unwrap_or("_"));
            if done >= len {
                return buf;
            }
        }
    }
}

/// Gets a pseudo random OsString of length `len` with characters in the
/// range [a-zA-Z0-9].
pub fn rand_alphanumerics(len: usize) -> OsString {
    rand_alphanumerics_impl(&xor_psuedo_rng_u32, len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_cycles() {
        for _ in 0..1000 {
            assert!(timestamp_cycles() < timestamp_cycles());
        }
    }

    #[test]
    fn test_xor_psuedo_rng_u32() {
        for _ in 0..1000 {
            assert_ne!(xor_psuedo_rng_u32(), xor_psuedo_rng_u32());
        }
    }

    #[test]
    fn test_xor_psuedo_rng_u8_alphas() {
        let i = 3612982; // 55 (shifted 16 places), 33 (shifted 8 places), 54...
                         // The 33 will be discarded as it is not a valid letter
                         // (upper or lower) or number.
        let s = xor_psuedo_rng_u8_alphanumerics(&|| i);
        assert_eq!(vec![54, 55], s);
    }

    #[test]
    fn test_rand_alphanumerics_impl() {
        let s = rand_alphanumerics_impl(&|| 14134, 5);
        assert_eq!("67676", s);
    }

    #[test]
    fn test_rand_alphanumerics() {
        let s = rand_alphanumerics(5);
        assert_eq!(5, s.len());
    }
}

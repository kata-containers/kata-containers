#![cfg(feature = "std")]

// This test uses procinfo, so can only be run on Linux.
extern crate libc;

use all_asserts::*;
use governor::{Quota, RateLimiter};
use nonzero_ext::*;
use std::sync::Arc;
use std::thread;

fn resident_memory_size() -> i64 {
    let mut out: libc::rusage = unsafe { std::mem::zeroed() };
    assert!(unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut out) } == 0);
    out.ru_maxrss
}

const LEAK_TOLERANCE: i64 = 1024 * 1024 * 10;

struct LeakCheck {
    usage_before: i64,
    n_iter: usize,
}

impl Drop for LeakCheck {
    fn drop(&mut self) {
        let usage_after = resident_memory_size();
        assert_le!(usage_after, self.usage_before + LEAK_TOLERANCE);
    }
}

impl LeakCheck {
    fn new(n_iter: usize) -> Self {
        LeakCheck {
            n_iter,
            usage_before: resident_memory_size(),
        }
    }
}

#[test]
fn memleak_gcra() {
    let bucket = RateLimiter::direct(Quota::per_second(nonzero!(1_000_000u32)));

    let leak_check = LeakCheck::new(500_000);

    for _i in 0..leak_check.n_iter {
        drop(bucket.check());
    }
}

#[test]
fn memleak_gcra_multi() {
    let bucket = RateLimiter::direct(Quota::per_second(nonzero!(1_000_000u32)));
    let leak_check = LeakCheck::new(500_000);

    for _i in 0..leak_check.n_iter {
        drop(bucket.check_n(nonzero!(2u32)));
    }
}

#[test]
fn memleak_gcra_threaded() {
    let bucket = Arc::new(RateLimiter::direct(Quota::per_second(nonzero!(
        1_000_000u32
    ))));
    let leak_check = LeakCheck::new(5_000);

    for _i in 0..leak_check.n_iter {
        let bucket = Arc::clone(&bucket);
        thread::spawn(move || {
            assert_eq!(Ok(()), bucket.check());
        })
        .join()
        .unwrap();
    }
}

#[test]
fn memleak_keyed() {
    let bucket = RateLimiter::keyed(Quota::per_second(nonzero!(50u32)));

    let leak_check = LeakCheck::new(500_000);

    for i in 0..leak_check.n_iter {
        drop(bucket.check_key(&(i % 1000)));
    }
}

//! Miscellaneous utilities.

use std::cell::{Cell, UnsafeCell};
use std::num::Wrapping;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_utils::Backoff;

/// Randomly shuffles a slice.
pub fn shuffle<T>(v: &mut [T]) {
    let len = v.len();
    if len <= 1 {
        return;
    }

    thread_local! {
        static RNG: Cell<Wrapping<u32>> = Cell::new(Wrapping(1406868647));
    }

    let _ = RNG.try_with(|rng| {
        for i in 1..len {
            // This is the 32-bit variant of Xorshift.
            //
            // Source: https://en.wikipedia.org/wiki/Xorshift
            let mut x = rng.get();
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            rng.set(x);

            let x = x.0;
            let n = i + 1;

            // This is a fast alternative to `let j = x % n`.
            //
            // Author: Daniel Lemire
            // Source: https://lemire.me/blog/2016/06/27/a-fast-alternative-to-the-modulo-reduction/
            let j = ((x as u64).wrapping_mul(n as u64) >> 32) as u32 as usize;

            v.swap(i, j);
        }
    });
}

/// Sleeps until the deadline, or forever if the deadline isn't specified.
pub fn sleep_until(deadline: Option<Instant>) {
    loop {
        match deadline {
            None => thread::sleep(Duration::from_secs(1000)),
            Some(d) => {
                let now = Instant::now();
                if now >= d {
                    break;
                }
                thread::sleep(d - now);
            }
        }
    }
}

/// A simple spinlock.
pub struct Spinlock<T> {
    flag: AtomicBool,
    value: UnsafeCell<T>,
}

impl<T> Spinlock<T> {
    /// Returns a new spinlock initialized with `value`.
    pub fn new(value: T) -> Spinlock<T> {
        Spinlock {
            flag: AtomicBool::new(false),
            value: UnsafeCell::new(value),
        }
    }

    /// Locks the spinlock.
    pub fn lock(&self) -> SpinlockGuard<'_, T> {
        let backoff = Backoff::new();
        while self.flag.swap(true, Ordering::Acquire) {
            backoff.snooze();
        }
        SpinlockGuard { parent: self }
    }
}

/// A guard holding a spinlock locked.
pub struct SpinlockGuard<'a, T: 'a> {
    parent: &'a Spinlock<T>,
}

impl<'a, T> Drop for SpinlockGuard<'a, T> {
    fn drop(&mut self) {
        self.parent.flag.store(false, Ordering::Release);
    }
}

impl<'a, T> Deref for SpinlockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.parent.value.get() }
    }
}

impl<'a, T> DerefMut for SpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.parent.value.get() }
    }
}

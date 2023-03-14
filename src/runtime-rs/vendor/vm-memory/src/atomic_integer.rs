// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

use std::sync::atomic::Ordering;

/// # Safety
///
/// Objects that implement this trait must consist exclusively of atomic types
/// from [`std::sync::atomic`](https://doc.rust-lang.org/std/sync/atomic/), except for
/// [`AtomicPtr<T>`](https://doc.rust-lang.org/std/sync/atomic/struct.AtomicPtr.html) and
/// [`AtomicBool`](https://doc.rust-lang.org/std/sync/atomic/struct.AtomicBool.html).
pub unsafe trait AtomicInteger: Sync + Send {
    /// The raw value type associated with the atomic integer (i.e. `u16` for `AtomicU16`).
    type V;

    /// Create a new instance of `Self`.
    fn new(v: Self::V) -> Self;

    /// Loads a value from the atomic integer.
    fn load(&self, order: Ordering) -> Self::V;

    /// Stores a value into the atomic integer.
    fn store(&self, val: Self::V, order: Ordering);
}

macro_rules! impl_atomic_integer_ops {
    ($T:path, $V:ty) => {
        unsafe impl AtomicInteger for $T {
            type V = $V;

            fn new(v: Self::V) -> Self {
                Self::new(v)
            }

            fn load(&self, order: Ordering) -> Self::V {
                self.load(order)
            }

            fn store(&self, val: Self::V, order: Ordering) {
                self.store(val, order)
            }
        }
    };
}

// TODO: Detect availability using #[cfg(target_has_atomic) when it is stabilized.
// Right now we essentially assume we're running on either x86 or Arm (32 or 64 bit). AFAIK,
// Rust starts using additional synchronization primitives to implement atomics when they're
// not natively available, and that doesn't interact safely with how we cast pointers to
// atomic value references. We should be wary of this when looking at a broader range of
// platforms.

impl_atomic_integer_ops!(std::sync::atomic::AtomicI8, i8);
impl_atomic_integer_ops!(std::sync::atomic::AtomicI16, i16);
impl_atomic_integer_ops!(std::sync::atomic::AtomicI32, i32);
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "s390x"
))]
impl_atomic_integer_ops!(std::sync::atomic::AtomicI64, i64);

impl_atomic_integer_ops!(std::sync::atomic::AtomicU8, u8);
impl_atomic_integer_ops!(std::sync::atomic::AtomicU16, u16);
impl_atomic_integer_ops!(std::sync::atomic::AtomicU32, u32);
#[cfg(any(
    target_arch = "x86_64",
    target_arch = "aarch64",
    target_arch = "powerpc64",
    target_arch = "s390x"
))]
impl_atomic_integer_ops!(std::sync::atomic::AtomicU64, u64);

impl_atomic_integer_ops!(std::sync::atomic::AtomicIsize, isize);
impl_atomic_integer_ops!(std::sync::atomic::AtomicUsize, usize);

#[cfg(test)]
mod tests {
    use super::*;

    use std::fmt::Debug;
    use std::sync::atomic::AtomicU32;

    fn check_atomic_integer_ops<A: AtomicInteger>()
    where
        A::V: Copy + Debug + From<u8> + PartialEq,
    {
        let v = A::V::from(0);
        let a = A::new(v);
        assert_eq!(a.load(Ordering::Relaxed), v);

        let v2 = A::V::from(100);
        a.store(v2, Ordering::Relaxed);
        assert_eq!(a.load(Ordering::Relaxed), v2);
    }

    #[test]
    fn test_atomic_integer_ops() {
        check_atomic_integer_ops::<AtomicU32>()
    }
}

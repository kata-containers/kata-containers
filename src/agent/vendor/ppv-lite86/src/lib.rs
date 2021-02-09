#![no_std]

// Design:
// - safety: safe creation of any machine type is done only by instance methods of a
//   Machine (which is a ZST + Copy type), which can only by created unsafely or safely
//   through feature detection (e.g. fn AVX2::try_get() -> Option<Machine>).

mod soft;
mod types;
pub use self::types::*;

#[cfg(all(feature = "simd", target_arch = "x86_64", not(miri)))]
pub mod x86_64;
#[cfg(all(feature = "simd", target_arch = "x86_64", not(miri)))]
use self::x86_64 as arch;

#[cfg(any(miri, not(all(feature = "simd", any(target_arch = "x86_64")))))]
pub mod generic;
#[cfg(any(miri, not(all(feature = "simd", any(target_arch = "x86_64")))))]
use self::generic as arch;

pub use self::arch::{vec128_storage, vec256_storage, vec512_storage};

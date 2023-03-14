#![deny(missing_docs, warnings)]

#![no_std]

//! `panic!()` in debug builds, optimization hint in release.

#[doc(hidden)] pub mod _internal { pub use core::hint::unreachable_unchecked; }

#[macro_export]
/// `panic!()` in debug builds, optimization hint in release.
///
/// Example:
///
/// ```
/// use debug_unreachable::debug_unreachable;
///
/// fn main() {
///     if 0 > 100 {
///         // Can't happen!
///         unsafe { debug_unreachable!() }
///     } else {
///         println!("Good, 0 <= 100.");
///     }
/// }
/// ```
macro_rules! debug_unreachable {
    () => { debug_unreachable!("entered unreachable code") };
    ($e:expr) => {
        if cfg!(debug_assertions) {
            panic!($e)
        } else {
            $crate::_internal::unreachable_unchecked()
        }
    }
}


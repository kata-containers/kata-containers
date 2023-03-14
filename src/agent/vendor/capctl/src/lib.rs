//! # `capctl`
//!
//! A library for manipulating Linux capabilities and making `prctl()` calls.
//!
//! # Potential Pitfalls
//!
//! - See [Handling of newly-added capabilities](#handling-of-newly-added-capabilities). **This can
//!   create security issues if it is not accounted for.**
//!
//! ## Handling of capabilities not supported by the kernel
//!
//! When a binary using this library is running on an older kernel that does not support a few
//! newly-added capabilities, here is how this library will handle them:
//!
//! - [`caps::Cap::is_supported()`] and [`caps::Cap::probe_supported()`] can be used to detect
//!   that the capability is unsupported (`cap.is_supported()` will return `false`, and
//!   `Cap::probe_supported()` will not include it in the returned set).
//! - [`caps::CapState`] and [`caps::FullCapState`] will never include the unsupported capability(s)
//!   in the returned capability sets.
//! - Trying to include the unsupported capability(s) in the new permitted/effective/inheritable
//!   sets with [`caps::CapState::set_current()`] will cause them to be silently removed from the
//!   new sets. (This is a kernel limitation.)
//! - The following functions will return an `Error` with code `EINVAL` if passed the unsupported
//!   capability:
//!   - [`caps::bounding::drop()`]
//!   - [`caps::ambient::raise()`]
//!   - [`caps::ambient::lower()`]
//! - [`caps::ambient::is_set()`] and [`caps::bounding::read()`] will return `None` if passed the
//!   unsupported capability.
//!
//! ## Handling of newly-added capabilities
//!
//! Conversely, when a binary using this library is running on a newer kernel that has added one or
//! more new capabilities, issues can arise. Here is how this library will handle those
//! capabilities:
//!
//! - If the permitted, effective, and/or inheritable capability sets of this process are modified
//!   (in any way) using [`caps::CapState`], the unknown capability(s) will be removed from the
//!   permitted, effective, and inheritable sets.
//! - The following functions are the **ONLY** functions in this crate that can be used to remove
//!   the unknown capability(s) from the ambient/bounding sets (see their documentation for more
//!   information):
//!   - [`caps::ambient::clear()`]
//!   - [`caps::ambient::clear_unknown()`]
//!   - [`caps::bounding::clear()`]
//!   - [`caps::bounding::clear_unknown()`]
//!
//! As a result, if you are trying to clear the ambient and/or bounding capability sets, you must
//! call the `clear()` or `clear_unknown()` function for whichever set you want to clear.

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]

mod err;
mod sys;

pub mod caps;
pub mod prctl;

pub use caps::*;
pub use err::*;
pub use prctl::*;

#[allow(clippy::needless_return)]
#[inline]
unsafe fn raw_prctl(
    option: libc::c_int,
    arg2: libc::c_ulong,
    arg3: libc::c_ulong,
    arg4: libc::c_ulong,
    arg5: libc::c_ulong,
) -> Result<libc::c_int> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "sc")] {
            return sc_res_decode(sc::syscall!(PRCTL, option, arg2, arg3, arg4, arg5))
                .map(|res| res as libc::c_int);
        } else {
            let res = libc::prctl(option, arg2, arg3, arg4, arg5);

            return if res >= 0 {
                Ok(res)
            } else {
                Err(Error::last())
            };
        }
    }
}

#[inline]
unsafe fn raw_prctl_opt(
    option: libc::c_int,
    arg2: libc::c_ulong,
    arg3: libc::c_ulong,
    arg4: libc::c_ulong,
    arg5: libc::c_ulong,
) -> Option<libc::c_int> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "sc")] {
            let res = sc::syscall!(PRCTL, option, arg2, arg3, arg4, arg5);

            if res <= -4096isize as usize {
                return Some(res as libc::c_int);
            }
        } else {
            let res = libc::prctl(option, arg2, arg3, arg4, arg5);

            if res >= 0 {
                return Some(res);
            }
        }
    }

    None
}

#[cfg(feature = "sc")]
#[inline]
fn sc_res_decode(res: usize) -> Result<usize> {
    if res > -4096isize as usize {
        Err(Error::from_code((!res + 1) as i32))
    } else {
        Ok(res)
    }
}

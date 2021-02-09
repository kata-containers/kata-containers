// Copyright 2017 Amanieu d'Antras
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! # unreachable
//! inlined from https://github.com/reem/rust-unreachable/
//!
//! An unreachable code optimization hint in stable rust, and some useful
//! extension traits for `Option` and `Result`.
//!

/// Hint to the optimizer that any code path which calls this function is
/// statically unreachable and can be removed.
///
/// Calling this function in reachable code invokes undefined behavior. Be
/// very, very sure this is what you want; often, a simple `panic!` is more
/// suitable.
#[inline]
pub unsafe fn unreachable() -> ! {
    /// The empty type for cases which can't occur.
    enum Void { }
    let x: &Void = ::std::mem::transmute(1usize);
    match *x {}
}

/// An extension trait for `Option<T>` providing unchecked unwrapping methods.
pub trait UncheckedOptionExt<T> {
    /// Get the value out of this Option without checking for None.
    unsafe fn unchecked_unwrap(self) -> T;

    /// Assert that this Option is a None to the optimizer.
    unsafe fn unchecked_unwrap_none(self);
}

/// An extension trait for `Result<T, E>` providing unchecked unwrapping methods.
pub trait UncheckedResultExt<T, E> {
    /// Get the value out of this Result without checking for Err.
    unsafe fn unchecked_unwrap_ok(self) -> T;

    /// Get the error out of this Result without checking for Ok.
    unsafe fn unchecked_unwrap_err(self) -> E;
}

impl<T> UncheckedOptionExt<T> for Option<T> {
    unsafe fn unchecked_unwrap(self) -> T {
        match self {
            Some(x) => x,
            None => unreachable()
        }
    }

    unsafe fn unchecked_unwrap_none(self) {
        if self.is_some() { unreachable() }
    }
}

impl<T, E> UncheckedResultExt<T, E> for Result<T, E> {
    unsafe fn unchecked_unwrap_ok(self) -> T {
        match self {
            Ok(x) => x,
            Err(_) => unreachable()
        }
    }

    unsafe fn unchecked_unwrap_err(self) -> E {
        match self {
            Ok(_) => unreachable(),
            Err(e) => e
        }
    }
}
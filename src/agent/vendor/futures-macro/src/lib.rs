//! The futures-rs procedural macro implementations.

#![recursion_limit = "128"]
#![warn(rust_2018_idioms, unreachable_pub)]
// It cannot be included in the published code because this lints have false positives in the minimum required version.
#![cfg_attr(test, warn(single_use_lifetimes))]
#![warn(clippy::all)]

// mem::take requires Rust 1.40, matches! requires Rust 1.42
// Can be removed if the minimum supported version increased or if https://github.com/rust-lang/rust-clippy/issues/3941
// get's implemented.
#![allow(clippy::mem_replace_with_default, clippy::match_like_matches_macro)]

#![doc(test(attr(deny(warnings), allow(dead_code, unused_assignments, unused_variables))))]

#![doc(html_root_url = "https://docs.rs/futures-join-macro/0.3.8")]

// Since https://github.com/rust-lang/cargo/pull/7700 `proc_macro` is part of the prelude for
// proc-macro crates, but to support older compilers we still need this explicit `extern crate`.
#[allow(unused_extern_crates)]
extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_hack::proc_macro_hack;

mod join;
mod select;

/// The `join!` macro.
#[proc_macro_hack]
pub fn join_internal(input: TokenStream) -> TokenStream {
    crate::join::join(input)
}

/// The `try_join!` macro.
#[proc_macro_hack]
pub fn try_join_internal(input: TokenStream) -> TokenStream {
    crate::join::try_join(input)
}

/// The `select!` macro.
#[proc_macro_hack]
pub fn select_internal(input: TokenStream) -> TokenStream {
    crate::select::select(input)
}

/// The `select_biased!` macro.
#[proc_macro_hack]
pub fn select_biased_internal(input: TokenStream) -> TokenStream {
    crate::select::select_biased(input)
}

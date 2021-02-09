#![allow(clippy::blocks_in_if_conditions, clippy::range_plus_one)]

extern crate proc_macro;

mod ast;
mod attr;
mod expand;
mod fmt;
mod prop;
mod valid;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Error, attributes(backtrace, error, from, source))]
pub fn derive_error(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand::derive(&input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

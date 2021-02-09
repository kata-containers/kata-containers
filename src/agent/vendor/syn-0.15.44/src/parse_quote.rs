/// Quasi-quotation macro that accepts input like the [`quote!`] macro but uses
/// type inference to figure out a return type for those tokens.
///
/// [`quote!`]: https://docs.rs/quote/0.6/quote/index.html
///
/// The return type can be any syntax tree node that implements the [`Parse`]
/// trait.
///
/// [`Parse`]: parse::Parse
///
/// ```edition2018
/// use quote::quote;
/// use syn::{parse_quote, Stmt};
///
/// fn main() {
///     let name = quote!(v);
///     let ty = quote!(u8);
///
///     let stmt: Stmt = parse_quote! {
///         let #name: #ty = Default::default();
///     };
///
///     println!("{:#?}", stmt);
/// }
/// ```
///
/// *This macro is available if Syn is built with the `"parsing"` feature,
/// although interpolation of syntax tree nodes into the quoted tokens is only
/// supported if Syn is built with the `"printing"` feature as well.*
///
/// # Example
///
/// The following helper function adds a bound `T: HeapSize` to every type
/// parameter `T` in the input generics.
///
/// ```edition2018
/// use syn::{parse_quote, Generics, GenericParam};
///
/// // Add a bound `T: HeapSize` to every type parameter T.
/// fn add_trait_bounds(mut generics: Generics) -> Generics {
///     for param in &mut generics.params {
///         if let GenericParam::Type(type_param) = param {
///             type_param.bounds.push(parse_quote!(HeapSize));
///         }
///     }
///     generics
/// }
/// ```
///
/// # Special cases
///
/// This macro can parse the following additional types as a special case even
/// though they do not implement the `Parse` trait.
///
/// - [`Attribute`] — parses one attribute, allowing either outer like `#[...]`
///   or inner like `#![...]`
/// - [`Punctuated<T, P>`] — parses zero or more `T` separated by punctuation
///   `P` with optional trailing punctuation
///
/// [`Punctuated<T, P>`]: punctuated::Punctuated
///
/// # Panics
///
/// Panics if the tokens fail to parse as the expected syntax tree type. The
/// caller is responsible for ensuring that the input tokens are syntactically
/// valid.
//
// TODO: allow Punctuated to be inferred as intra doc link, currently blocked on
// https://github.com/rust-lang/rust/issues/62834
#[macro_export(local_inner_macros)]
macro_rules! parse_quote {
    ($($tt:tt)*) => {
        $crate::parse_quote::parse($crate::export::From::from(quote_impl!($($tt)*)))
    };
}

#[cfg(not(syn_can_call_macro_by_path))]
#[doc(hidden)]
#[macro_export]
macro_rules! quote_impl {
    ($($tt:tt)*) => {
        // Require caller to have their own `#[macro_use] extern crate quote`.
        quote!($($tt)*)
    };
}

#[cfg(syn_can_call_macro_by_path)]
#[doc(hidden)]
#[macro_export]
macro_rules! quote_impl {
    ($($tt:tt)*) => {
        $crate::export::quote::quote!($($tt)*)
    };
}

////////////////////////////////////////////////////////////////////////////////
// Can parse any type that implements Parse.

use parse::{Parse, ParseStream, Parser, Result};
use proc_macro2::TokenStream;

// Not public API.
#[doc(hidden)]
pub fn parse<T: ParseQuote>(token_stream: TokenStream) -> T {
    let parser = T::parse;
    match parser.parse2(token_stream) {
        Ok(t) => t,
        Err(err) => panic!("{}", err),
    }
}

// Not public API.
#[doc(hidden)]
pub trait ParseQuote: Sized {
    fn parse(input: ParseStream) -> Result<Self>;
}

impl<T: Parse> ParseQuote for T {
    fn parse(input: ParseStream) -> Result<Self> {
        <T as Parse>::parse(input)
    }
}

////////////////////////////////////////////////////////////////////////////////
// Any other types that we want `parse_quote!` to be able to parse.

use punctuated::Punctuated;
#[cfg(any(feature = "full", feature = "derive"))]
use {attr, Attribute};

#[cfg(any(feature = "full", feature = "derive"))]
impl ParseQuote for Attribute {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![#]) && input.peek2(Token![!]) {
            attr::parsing::single_parse_inner(input)
        } else {
            attr::parsing::single_parse_outer(input)
        }
    }
}

impl<T: Parse, P: Parse> ParseQuote for Punctuated<T, P> {
    fn parse(input: ParseStream) -> Result<Self> {
        Self::parse_terminated(input)
    }
}

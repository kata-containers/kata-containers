#![deny(
    anonymous_parameters,
    clippy::all,
    const_err,
    illegal_floating_point_literal_pattern,
    late_bound_lifetime_arguments,
    path_statements,
    patterns_in_fns_without_body,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    unused_extern_crates
)]
#![warn(
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    clippy::get_unwrap,
    clippy::nursery,
    clippy::print_stdout,
    clippy::todo,
    clippy::unimplemented,
    clippy::unnested_or_patterns,
    clippy::unwrap_used,
    clippy::use_debug,
    single_use_lifetimes,
    unused_qualifications,
    variant_size_differences
)]
#![allow(
    clippy::missing_const_for_fn, // useless in proc macro
    clippy::redundant_pub_crate, // suggests bad style
    clippy::option_if_let_else, // suggests terrible code
)]

#[macro_use]
mod quote;

mod date;
mod datetime;
mod error;
#[cfg(any(feature = "formatting", feature = "parsing"))]
mod format_description;
mod helpers;
mod offset;
#[cfg(all(feature = "serde", any(feature = "formatting", feature = "parsing")))]
mod serde_format_description;
mod time;
mod to_tokens;

use proc_macro::TokenStream;
#[cfg(all(feature = "serde", any(feature = "formatting", feature = "parsing")))]
use proc_macro::TokenTree;

use self::error::Error;

macro_rules! impl_macros {
    ($($name:ident)*) => {$(
        #[proc_macro]
        pub fn $name(input: TokenStream) -> TokenStream {
            use crate::to_tokens::ToTokenTree;

            let mut iter = input.into_iter().peekable();
            match $name::parse(&mut iter) {
                Ok(value) => match iter.peek() {
                    Some(tree) => Error::UnexpectedToken { tree: tree.clone() }.to_compile_error(),
                    None => TokenStream::from(value.into_token_tree()),
                },
                Err(err) => err.to_compile_error(),
            }
        }
    )*};
}

impl_macros![date datetime offset time];

#[cfg(any(feature = "formatting", feature = "parsing"))]
#[proc_macro]
pub fn format_description(input: TokenStream) -> TokenStream {
    (|| {
        let (span, string) = helpers::get_string_literal(input)?;
        let items = format_description::parse(&string, span)?;

        Ok(quote! {{
            const DESCRIPTION: &[::time::format_description::FormatItem<'_>] = &[#S(
                items
                    .into_iter()
                    .map(|item| quote! { #S(item), })
                    .collect::<TokenStream>()
            )];
            DESCRIPTION
        }})
    })()
    .unwrap_or_else(|err: Error| err.to_compile_error())
}

#[cfg(all(feature = "serde", any(feature = "formatting", feature = "parsing")))]
#[proc_macro]
pub fn serde_format_description(input: TokenStream) -> TokenStream {
    (|| {
        let mut tokens = input.into_iter().peekable();
        // First, an identifier (the desired module name)
        let mod_name = match tokens.next() {
            Some(TokenTree::Ident(ident)) => Ok(ident),
            Some(tree) => Err(Error::UnexpectedToken { tree }),
            None => Err(Error::UnexpectedEndOfInput),
        }?;

        // Followed by a comma
        helpers::consume_punct(',', &mut tokens)?;

        // Then, the type to create serde serializers for (e.g., `OffsetDateTime`).
        let formattable = match tokens.next() {
            Some(tree @ TokenTree::Ident(_)) => Ok(tree),
            Some(tree) => Err(Error::UnexpectedToken { tree }),
            None => Err(Error::UnexpectedEndOfInput),
        }?;

        // Another comma
        helpers::consume_punct(',', &mut tokens)?;

        // We now have two options. The user can either provide a format description as a string or
        // they can provide a path to a format description. If the latter, all remaining tokens are
        // assumed to be part of the path.
        let (format, raw_format_string) = match tokens.peek() {
            // string literal
            Some(TokenTree::Literal(_)) => {
                let (span, format_string) = helpers::get_string_literal(tokens.collect())?;
                let items = format_description::parse(&format_string, span)?;
                let items: TokenStream =
                    items.into_iter().map(|item| quote! { #S(item), }).collect();
                let items = quote! { &[#S(items)] };

                (
                    items,
                    Some(String::from_utf8_lossy(&format_string).into_owned()),
                )
            }
            // path
            Some(_) => (
                quote! {{
                    // We can't just do `super::path` because the path could be an absolute path. In
                    // that case, we'd be generating `super::::path`, which is invalid. Even if we
                    // took that into account, it's not possible to know if it's an external crate,
                    // which would just require emitting `path` directly. By taking this approach,
                    // we can leave it to the compiler to do the actual resolution.
                    mod __path_hack {
                        pub(super) use super::super::*;
                        pub(super) use #S(tokens.collect::<TokenStream>()) as FORMAT;
                    }
                    __path_hack::FORMAT
                }},
                None,
            ),
            None => return Err(Error::UnexpectedEndOfInput),
        };

        Ok(serde_format_description::build(
            mod_name,
            formattable,
            format,
            raw_format_string,
        ))
    })()
    .unwrap_or_else(|err: Error| err.to_compile_error_standalone())
}

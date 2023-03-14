use proc_macro2::Span;
use syn::parse::{Error, Parse, ParseStream, Result};
use syn::{token::Question, ItemFn, Token};

pub struct AsyncItem(pub ItemFn);

impl Parse for AsyncItem {
    fn parse(input: ParseStream) -> Result<Self> {
        let item: ItemFn = input.parse()?;

        // Check that this is an async function
        if item.sig.asyncness.is_none() {
            return Err(Error::new(Span::call_site(), "expected an async function"));
        }

        Ok(AsyncItem(item))
    }
}

pub struct RecursionArgs {
    pub send_bound: bool,
}

impl Default for RecursionArgs {
    fn default() -> Self {
        RecursionArgs { send_bound: true }
    }
}

/// Custom keywords for parser
mod kw {
    syn::custom_keyword!(Send);
}

impl Parse for RecursionArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        // Check for the `?Send` option
        if input.peek(Token![?]) {
            input.parse::<Question>()?;
            input.parse::<kw::Send>()?;
            Ok(Self { send_bound: false })
        } else if !input.is_empty() {
            Err(input.error("expected `?Send` or empty"))
        } else {
            Ok(Self::default())
        }
    }
}

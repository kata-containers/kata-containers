mod component;
pub(crate) mod error;
pub(crate) mod modifier;
pub(crate) mod parse;

use proc_macro::{Literal, TokenStream};

pub(crate) use self::component::Component;
pub(crate) use self::parse::parse;
use crate::to_tokens::ToTokenStream;

mod helper {
    #[must_use = "This does not modify the original slice."]
    pub(crate) fn consume_whitespace<'a>(bytes: &'a [u8], index: &mut usize) -> &'a [u8] {
        let first_non_whitespace = bytes
            .iter()
            .position(|c| !c.is_ascii_whitespace())
            .unwrap_or(bytes.len());
        *index += first_non_whitespace;
        &bytes[first_non_whitespace..]
    }
}

#[allow(single_use_lifetimes)] // false positive
#[allow(variant_size_differences)]
pub(crate) enum FormatItem<'a> {
    Literal(&'a [u8]),
    Component(Component),
}

impl ToTokenStream for FormatItem<'_> {
    fn append_to(self, ts: &mut TokenStream) {
        quote_append! { ts
            ::time::format_description::FormatItem::#S(match self {
                FormatItem::Literal(bytes) => quote! { Literal(#(Literal::byte_string(bytes))) },
                FormatItem::Component(component) => quote! { Component(#S(component)) },
            })
        }
    }
}

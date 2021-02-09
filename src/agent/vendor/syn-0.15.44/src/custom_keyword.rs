/// Define a type that supports parsing and printing a given identifier as if it
/// were a keyword.
///
/// # Usage
///
/// As a convention, it is recommended that this macro be invoked within a
/// module called `kw` or `keyword` and that the resulting parser be invoked
/// with a `kw::` or `keyword::` prefix.
///
/// ```edition2018
/// mod kw {
///     syn::custom_keyword!(whatever);
/// }
/// ```
///
/// The generated syntax tree node supports the following operations just like
/// any built-in keyword token.
///
/// - [Peeking] — `input.peek(kw::whatever)`
///
/// - [Parsing] — `input.parse::<kw::whatever>()?`
///
/// - [Printing] — `quote!( ... #whatever_token ... )`
///
/// - Construction from a [`Span`] — `let whatever_token = kw::whatever(sp)`
///
/// - Field access to its span — `let sp = whatever_token.span`
///
/// [Peeking]: parse::ParseBuffer::peek
/// [Parsing]: parse::ParseBuffer::parse
/// [Printing]: quote::ToTokens
/// [`Span`]: proc_macro2::Span
///
/// # Example
///
/// This example parses input that looks like `bool = true` or `str = "value"`.
/// The key must be either the identifier `bool` or the identifier `str`. If
/// `bool`, the value may be either `true` or `false`. If `str`, the value may
/// be any string literal.
///
/// The symbols `bool` and `str` are not reserved keywords in Rust so these are
/// not considered keywords in the `syn::token` module. Like any other
/// identifier that is not a keyword, these can be declared as custom keywords
/// by crates that need to use them as such.
///
/// ```edition2018
/// use syn::{LitBool, LitStr, Result, Token};
/// use syn::parse::{Parse, ParseStream};
///
/// mod kw {
///     syn::custom_keyword!(bool);
///     syn::custom_keyword!(str);
/// }
///
/// enum Argument {
///     Bool {
///         bool_token: kw::bool,
///         eq_token: Token![=],
///         value: LitBool,
///     },
///     Str {
///         str_token: kw::str,
///         eq_token: Token![=],
///         value: LitStr,
///     },
/// }
///
/// impl Parse for Argument {
///     fn parse(input: ParseStream) -> Result<Self> {
///         let lookahead = input.lookahead1();
///         if lookahead.peek(kw::bool) {
///             Ok(Argument::Bool {
///                 bool_token: input.parse::<kw::bool>()?,
///                 eq_token: input.parse()?,
///                 value: input.parse()?,
///             })
///         } else if lookahead.peek(kw::str) {
///             Ok(Argument::Str {
///                 str_token: input.parse::<kw::str>()?,
///                 eq_token: input.parse()?,
///                 value: input.parse()?,
///             })
///         } else {
///             Err(lookahead.error())
///         }
///     }
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! custom_keyword {
    ($ident:ident) => {
        #[allow(non_camel_case_types)]
        pub struct $ident {
            pub span: $crate::export::Span,
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub fn $ident<__S: $crate::export::IntoSpans<[$crate::export::Span; 1]>>(
            span: __S,
        ) -> $ident {
            $ident {
                span: $crate::export::IntoSpans::into_spans(span)[0],
            }
        }

        impl $crate::export::Default for $ident {
            fn default() -> Self {
                $ident {
                    span: $crate::export::Span::call_site(),
                }
            }
        }

        impl_parse_for_custom_keyword!($ident);
        impl_to_tokens_for_custom_keyword!($ident);
        impl_clone_for_custom_keyword!($ident);
        impl_extra_traits_for_custom_keyword!($ident);
    };
}

// Not public API.
#[cfg(feature = "parsing")]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_parse_for_custom_keyword {
    ($ident:ident) => {
        // For peek.
        impl $crate::token::CustomToken for $ident {
            fn peek(cursor: $crate::buffer::Cursor) -> $crate::export::bool {
                if let Some((ident, _rest)) = cursor.ident() {
                    ident == stringify!($ident)
                } else {
                    false
                }
            }

            fn display() -> &'static $crate::export::str {
                concat!("`", stringify!($ident), "`")
            }
        }

        impl $crate::parse::Parse for $ident {
            fn parse(input: $crate::parse::ParseStream) -> $crate::parse::Result<$ident> {
                input.step(|cursor| {
                    if let $crate::export::Some((ident, rest)) = cursor.ident() {
                        if ident == stringify!($ident) {
                            return $crate::export::Ok(($ident { span: ident.span() }, rest));
                        }
                    }
                    $crate::export::Err(cursor.error(concat!(
                        "expected `",
                        stringify!($ident),
                        "`"
                    )))
                })
            }
        }
    };
}

// Not public API.
#[cfg(not(feature = "parsing"))]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_parse_for_custom_keyword {
    ($ident:ident) => {};
}

// Not public API.
#[cfg(feature = "printing")]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_to_tokens_for_custom_keyword {
    ($ident:ident) => {
        impl $crate::export::ToTokens for $ident {
            fn to_tokens(&self, tokens: &mut $crate::export::TokenStream2) {
                let ident = $crate::Ident::new(stringify!($ident), self.span);
                $crate::export::TokenStreamExt::append(tokens, ident);
            }
        }
    };
}

// Not public API.
#[cfg(not(feature = "printing"))]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_to_tokens_for_custom_keyword {
    ($ident:ident) => {};
}

// Not public API.
#[cfg(feature = "clone-impls")]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_clone_for_custom_keyword {
    ($ident:ident) => {
        impl $crate::export::Copy for $ident {}

        impl $crate::export::Clone for $ident {
            fn clone(&self) -> Self {
                *self
            }
        }
    };
}

// Not public API.
#[cfg(not(feature = "clone-impls"))]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_clone_for_custom_keyword {
    ($ident:ident) => {};
}

// Not public API.
#[cfg(feature = "extra-traits")]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_extra_traits_for_custom_keyword {
    ($ident:ident) => {
        impl $crate::export::Debug for $ident {
            fn fmt(&self, f: &mut $crate::export::Formatter) -> $crate::export::fmt::Result {
                $crate::export::Formatter::write_str(
                    f,
                    concat!("Keyword [", stringify!($ident), "]"),
                )
            }
        }

        impl $crate::export::Eq for $ident {}

        impl $crate::export::PartialEq for $ident {
            fn eq(&self, _other: &Self) -> $crate::export::bool {
                true
            }
        }

        impl $crate::export::Hash for $ident {
            fn hash<__H: $crate::export::Hasher>(&self, _state: &mut __H) {}
        }
    };
}

// Not public API.
#[cfg(not(feature = "extra-traits"))]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_extra_traits_for_custom_keyword {
    ($ident:ident) => {};
}

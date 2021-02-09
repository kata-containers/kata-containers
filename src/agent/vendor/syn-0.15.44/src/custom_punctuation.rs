/// Define a type that supports parsing and printing a multi-character symbol
/// as if it were a punctuation token.
///
/// # Usage
///
/// ```edition2018
/// syn::custom_punctuation!(LeftRightArrow, <=>);
/// ```
///
/// The generated syntax tree node supports the following operations just like
/// any built-in punctuation token.
///
/// - [Peeking] — `input.peek(LeftRightArrow)`
///
/// - [Parsing] — `input.parse::<LeftRightArrow>()?`
///
/// - [Printing] — `quote!( ... #lrarrow ... )`
///
/// - Construction from a [`Span`] — `let lrarrow = LeftRightArrow(sp)`
///
/// - Construction from multiple [`Span`] — `let lrarrow = LeftRightArrow([sp, sp, sp])`
///
/// - Field access to its spans — `let spans = lrarrow.spans`
///
/// [Peeking]: parse::ParseBuffer::peek
/// [Parsing]: parse::ParseBuffer::parse
/// [Printing]: quote::ToTokens
/// [`Span`]: proc_macro2::Span
///
/// # Example
///
/// ```edition2018
/// use proc_macro2::{TokenStream, TokenTree};
/// use syn::parse::{Parse, ParseStream, Peek, Result};
/// use syn::punctuated::Punctuated;
/// use syn::Expr;
///
/// syn::custom_punctuation!(PathSeparator, </>);
///
/// // expr </> expr </> expr ...
/// struct PathSegments {
///     segments: Punctuated<Expr, PathSeparator>,
/// }
///
/// impl Parse for PathSegments {
///     fn parse(input: ParseStream) -> Result<Self> {
///         let mut segments = Punctuated::new();
///
///         let first = parse_until(input, PathSeparator)?;
///         segments.push_value(syn::parse2(first)?);
///
///         while input.peek(PathSeparator) {
///             segments.push_punct(input.parse()?);
///
///             let next = parse_until(input, PathSeparator)?;
///             segments.push_value(syn::parse2(next)?);
///         }
///
///         Ok(PathSegments { segments })
///     }
/// }
///
/// fn parse_until<E: Peek>(input: ParseStream, end: E) -> Result<TokenStream> {
///     let mut tokens = TokenStream::new();
///     while !input.is_empty() && !input.peek(end) {
///         let next: TokenTree = input.parse()?;
///         tokens.extend(Some(next));
///     }
///     Ok(tokens)
/// }
///
/// fn main() {
///     let input = r#" a::b </> c::d::e "#;
///     let _: PathSegments = syn::parse_str(input).unwrap();
/// }
/// ```
#[macro_export(local_inner_macros)]
macro_rules! custom_punctuation {
    ($ident:ident, $($tt:tt)+) => {
        pub struct $ident {
            pub spans: custom_punctuation_repr!($($tt)+),
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        pub fn $ident<__S: $crate::export::IntoSpans<custom_punctuation_repr!($($tt)+)>>(
            spans: __S,
        ) -> $ident {
            let _validate_len = 0 $(+ custom_punctuation_len!(strict, $tt))*;
            $ident {
                spans: $crate::export::IntoSpans::into_spans(spans)
            }
        }

        impl $crate::export::Default for $ident {
            fn default() -> Self {
                $ident($crate::export::Span::call_site())
            }
        }

        impl_parse_for_custom_punctuation!($ident, $($tt)+);
        impl_to_tokens_for_custom_punctuation!($ident, $($tt)+);
        impl_clone_for_custom_punctuation!($ident, $($tt)+);
        impl_extra_traits_for_custom_punctuation!($ident, $($tt)+);
    };
}

// Not public API.
#[cfg(feature = "parsing")]
#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! impl_parse_for_custom_punctuation {
    ($ident:ident, $($tt:tt)+) => {
        impl $crate::token::CustomToken for $ident {
            fn peek(cursor: $crate::buffer::Cursor) -> bool {
                $crate::token::parsing::peek_punct(cursor, stringify_punct!($($tt)+))
            }

            fn display() -> &'static $crate::export::str {
                custom_punctuation_concat!("`", stringify_punct!($($tt)+), "`")
            }
        }

        impl $crate::parse::Parse for $ident {
            fn parse(input: $crate::parse::ParseStream) -> $crate::parse::Result<$ident> {
                let spans: custom_punctuation_repr!($($tt)+) =
                    $crate::token::parsing::punct(input, stringify_punct!($($tt)+))?;
                Ok($ident(spans))
            }
        }
    };
}

// Not public API.
#[cfg(not(feature = "parsing"))]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_parse_for_custom_punctuation {
    ($ident:ident, $($tt:tt)+) => {};
}

// Not public API.
#[cfg(feature = "printing")]
#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! impl_to_tokens_for_custom_punctuation {
    ($ident:ident, $($tt:tt)+) => {
        impl $crate::export::ToTokens for $ident {
            fn to_tokens(&self, tokens: &mut $crate::export::TokenStream2) {
                $crate::token::printing::punct(stringify_punct!($($tt)+), &self.spans, tokens)
            }
        }
    };
}

// Not public API.
#[cfg(not(feature = "printing"))]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_to_tokens_for_custom_punctuation {
    ($ident:ident, $($tt:tt)+) => {};
}

// Not public API.
#[cfg(feature = "clone-impls")]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_clone_for_custom_punctuation {
    ($ident:ident, $($tt:tt)+) => {
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
macro_rules! impl_clone_for_custom_punctuation {
    ($ident:ident, $($tt:tt)+) => {};
}

// Not public API.
#[cfg(feature = "extra-traits")]
#[doc(hidden)]
#[macro_export]
macro_rules! impl_extra_traits_for_custom_punctuation {
    ($ident:ident, $($tt:tt)+) => {
        impl $crate::export::Debug for $ident {
            fn fmt(&self, f: &mut $crate::export::Formatter) -> $crate::export::fmt::Result {
                $crate::export::Formatter::write_str(f, stringify!($ident))
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
macro_rules! impl_extra_traits_for_custom_punctuation {
    ($ident:ident, $($tt:tt)+) => {};
}

// Not public API.
#[doc(hidden)]
#[macro_export(local_inner_macros)]
macro_rules! custom_punctuation_repr {
    ($($tt:tt)+) => {
        [$crate::export::Span; 0 $(+ custom_punctuation_len!(lenient, $tt))+]
    };
}

// Not public API.
#[doc(hidden)]
#[macro_export(local_inner_macros)]
#[cfg_attr(rustfmt, rustfmt_skip)]
macro_rules! custom_punctuation_len {
    ($mode:ident, +)     => { 1 };
    ($mode:ident, +=)    => { 2 };
    ($mode:ident, &)     => { 1 };
    ($mode:ident, &&)    => { 2 };
    ($mode:ident, &=)    => { 2 };
    ($mode:ident, @)     => { 1 };
    ($mode:ident, !)     => { 1 };
    ($mode:ident, ^)     => { 1 };
    ($mode:ident, ^=)    => { 2 };
    ($mode:ident, :)     => { 1 };
    ($mode:ident, ::)    => { 2 };
    ($mode:ident, ,)     => { 1 };
    ($mode:ident, /)     => { 1 };
    ($mode:ident, /=)    => { 2 };
    ($mode:ident, .)     => { 1 };
    ($mode:ident, ..)    => { 2 };
    ($mode:ident, ...)   => { 3 };
    ($mode:ident, ..=)   => { 3 };
    ($mode:ident, =)     => { 1 };
    ($mode:ident, ==)    => { 2 };
    ($mode:ident, >=)    => { 2 };
    ($mode:ident, >)     => { 1 };
    ($mode:ident, <=)    => { 2 };
    ($mode:ident, <)     => { 1 };
    ($mode:ident, *=)    => { 2 };
    ($mode:ident, !=)    => { 2 };
    ($mode:ident, |)     => { 1 };
    ($mode:ident, |=)    => { 2 };
    ($mode:ident, ||)    => { 2 };
    ($mode:ident, #)     => { 1 };
    ($mode:ident, ?)     => { 1 };
    ($mode:ident, ->)    => { 2 };
    ($mode:ident, <-)    => { 2 };
    ($mode:ident, %)     => { 1 };
    ($mode:ident, %=)    => { 2 };
    ($mode:ident, =>)    => { 2 };
    ($mode:ident, ;)     => { 1 };
    ($mode:ident, <<)    => { 2 };
    ($mode:ident, <<=)   => { 3 };
    ($mode:ident, >>)    => { 2 };
    ($mode:ident, >>=)   => { 3 };
    ($mode:ident, *)     => { 1 };
    ($mode:ident, -)     => { 1 };
    ($mode:ident, -=)    => { 2 };
    ($mode:ident, ~)     => { 1 };
    (lenient, $tt:tt)    => { 0 };
    (strict, $tt:tt)     => {{ custom_punctuation_unexpected!($tt); 0 }};
}

// Not public API.
#[doc(hidden)]
#[macro_export]
macro_rules! custom_punctuation_unexpected {
    () => {};
}

// Not public API.
#[doc(hidden)]
#[macro_export]
macro_rules! stringify_punct {
    ($($tt:tt)+) => {
        concat!($(stringify!($tt)),+)
    };
}

// Not public API.
// Without this, local_inner_macros breaks when looking for concat!
#[doc(hidden)]
#[macro_export]
macro_rules! custom_punctuation_concat {
    ($($tt:tt)*) => {
        concat!($($tt)*)
    };
}

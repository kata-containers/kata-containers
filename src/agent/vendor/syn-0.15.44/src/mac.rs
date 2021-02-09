use super::*;
use proc_macro2::TokenStream;
#[cfg(feature = "parsing")]
use proc_macro2::{Delimiter, Span, TokenTree};
use token::{Brace, Bracket, Paren};

#[cfg(feature = "parsing")]
use parse::{Parse, ParseStream, Parser, Result};
#[cfg(feature = "extra-traits")]
use std::hash::{Hash, Hasher};
#[cfg(feature = "extra-traits")]
use tt::TokenStreamHelper;

ast_struct! {
    /// A macro invocation: `println!("{}", mac)`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub struct Macro #manual_extra_traits {
        pub path: Path,
        pub bang_token: Token![!],
        pub delimiter: MacroDelimiter,
        pub tts: TokenStream,
    }
}

ast_enum! {
    /// A grouping token that surrounds a macro body: `m!(...)` or `m!{...}` or `m![...]`.
    ///
    /// *This type is available if Syn is built with the `"derive"` or `"full"`
    /// feature.*
    pub enum MacroDelimiter {
        Paren(Paren),
        Brace(Brace),
        Bracket(Bracket),
    }
}

#[cfg(feature = "extra-traits")]
impl Eq for Macro {}

#[cfg(feature = "extra-traits")]
impl PartialEq for Macro {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.bang_token == other.bang_token
            && self.delimiter == other.delimiter
            && TokenStreamHelper(&self.tts) == TokenStreamHelper(&other.tts)
    }
}

#[cfg(feature = "extra-traits")]
impl Hash for Macro {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        self.path.hash(state);
        self.bang_token.hash(state);
        self.delimiter.hash(state);
        TokenStreamHelper(&self.tts).hash(state);
    }
}

#[cfg(feature = "parsing")]
fn delimiter_span(delimiter: &MacroDelimiter) -> Span {
    match *delimiter {
        MacroDelimiter::Paren(ref token) => token.span,
        MacroDelimiter::Brace(ref token) => token.span,
        MacroDelimiter::Bracket(ref token) => token.span,
    }
}

impl Macro {
    /// Parse the tokens within the macro invocation's delimiters into a syntax
    /// tree.
    ///
    /// This is equivalent to `syn::parse2::<T>(mac.tts)` except that it
    /// produces a more useful span when `tts` is empty.
    ///
    /// # Example
    ///
    /// ```edition2018
    /// use syn::{parse_quote, Expr, ExprLit, Ident, Lit, LitStr, Macro, Token};
    /// use syn::ext::IdentExt;
    /// use syn::parse::{Error, Parse, ParseStream, Result};
    /// use syn::punctuated::Punctuated;
    ///
    /// // The arguments expected by libcore's format_args macro, and as a
    /// // result most other formatting and printing macros like println.
    /// //
    /// //     println!("{} is {number:.prec$}", "x", prec=5, number=0.01)
    /// struct FormatArgs {
    ///     format_string: Expr,
    ///     positional_args: Vec<Expr>,
    ///     named_args: Vec<(Ident, Expr)>,
    /// }
    ///
    /// impl Parse for FormatArgs {
    ///     fn parse(input: ParseStream) -> Result<Self> {
    ///         let format_string: Expr;
    ///         let mut positional_args = Vec::new();
    ///         let mut named_args = Vec::new();
    ///
    ///         format_string = input.parse()?;
    ///         while !input.is_empty() {
    ///             input.parse::<Token![,]>()?;
    ///             if input.is_empty() {
    ///                 break;
    ///             }
    ///             if input.peek(Ident::peek_any) && input.peek2(Token![=]) {
    ///                 while !input.is_empty() {
    ///                     let name: Ident = input.call(Ident::parse_any)?;
    ///                     input.parse::<Token![=]>()?;
    ///                     let value: Expr = input.parse()?;
    ///                     named_args.push((name, value));
    ///                     if input.is_empty() {
    ///                         break;
    ///                     }
    ///                     input.parse::<Token![,]>()?;
    ///                 }
    ///                 break;
    ///             }
    ///             positional_args.push(input.parse()?);
    ///         }
    ///
    ///         Ok(FormatArgs {
    ///             format_string,
    ///             positional_args,
    ///             named_args,
    ///         })
    ///     }
    /// }
    ///
    /// // Extract the first argument, the format string literal, from an
    /// // invocation of a formatting or printing macro.
    /// fn get_format_string(m: &Macro) -> Result<LitStr> {
    ///     let args: FormatArgs = m.parse_body()?;
    ///     match args.format_string {
    ///         Expr::Lit(ExprLit { lit: Lit::Str(lit), .. }) => Ok(lit),
    ///         other => {
    ///             // First argument was not a string literal expression.
    ///             // Maybe something like: println!(concat!(...), ...)
    ///             Err(Error::new_spanned(other, "format string must be a string literal"))
    ///         }
    ///     }
    /// }
    ///
    /// fn main() {
    ///     let invocation = parse_quote! {
    ///         println!("{:?}", Instant::now())
    ///     };
    ///     let lit = get_format_string(&invocation).unwrap();
    ///     assert_eq!(lit.value(), "{:?}");
    /// }
    /// ```
    #[cfg(feature = "parsing")]
    pub fn parse_body<T: Parse>(&self) -> Result<T> {
        self.parse_body_with(T::parse)
    }

    /// Parse the tokens within the macro invocation's delimiters using the
    /// given parser.
    #[cfg(feature = "parsing")]
    pub fn parse_body_with<F: Parser>(&self, parser: F) -> Result<F::Output> {
        // TODO: see if we can get a group.span_close() span in here as the
        // scope, rather than the span of the whole group.
        let scope = delimiter_span(&self.delimiter);
        private::parse_scoped(parser, scope, self.tts.clone())
    }
}

#[cfg(feature = "parsing")]
pub fn parse_delimiter(input: ParseStream) -> Result<(MacroDelimiter, TokenStream)> {
    input.step(|cursor| {
        if let Some((TokenTree::Group(g), rest)) = cursor.token_tree() {
            let span = g.span();
            let delimiter = match g.delimiter() {
                Delimiter::Parenthesis => MacroDelimiter::Paren(Paren(span)),
                Delimiter::Brace => MacroDelimiter::Brace(Brace(span)),
                Delimiter::Bracket => MacroDelimiter::Bracket(Bracket(span)),
                Delimiter::None => {
                    return Err(cursor.error("expected delimiter"));
                }
            };
            Ok(((delimiter, g.stream().clone()), rest))
        } else {
            Err(cursor.error("expected delimiter"))
        }
    })
}

#[cfg(feature = "parsing")]
pub mod parsing {
    use super::*;

    use parse::{Parse, ParseStream, Result};

    impl Parse for Macro {
        fn parse(input: ParseStream) -> Result<Self> {
            let tts;
            Ok(Macro {
                path: input.call(Path::parse_mod_style)?,
                bang_token: input.parse()?,
                delimiter: {
                    let (delimiter, content) = parse_delimiter(input)?;
                    tts = content;
                    delimiter
                },
                tts: tts,
            })
        }
    }
}

#[cfg(feature = "printing")]
mod printing {
    use super::*;
    use proc_macro2::TokenStream;
    use quote::ToTokens;

    impl ToTokens for Macro {
        fn to_tokens(&self, tokens: &mut TokenStream) {
            self.path.to_tokens(tokens);
            self.bang_token.to_tokens(tokens);
            match self.delimiter {
                MacroDelimiter::Paren(ref paren) => {
                    paren.surround(tokens, |tokens| self.tts.to_tokens(tokens));
                }
                MacroDelimiter::Brace(ref brace) => {
                    brace.surround(tokens, |tokens| self.tts.to_tokens(tokens));
                }
                MacroDelimiter::Bracket(ref bracket) => {
                    bracket.surround(tokens, |tokens| self.tts.to_tokens(tokens));
                }
            }
        }
    }
}

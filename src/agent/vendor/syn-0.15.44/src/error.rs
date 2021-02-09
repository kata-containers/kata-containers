use std;
use std::fmt::{self, Debug, Display};
use std::iter::FromIterator;

use proc_macro2::{
    Delimiter, Group, Ident, LexError, Literal, Punct, Spacing, Span, TokenStream, TokenTree,
};
#[cfg(feature = "printing")]
use quote::ToTokens;

#[cfg(feature = "parsing")]
use buffer::Cursor;
#[cfg(all(procmacro2_semver_exempt, feature = "parsing"))]
use private;
use thread::ThreadBound;

/// The result of a Syn parser.
pub type Result<T> = std::result::Result<T, Error>;

/// Error returned when a Syn parser cannot parse the input tokens.
///
/// Refer to the [module documentation] for details about parsing in Syn.
///
/// [module documentation]: index.html
///
/// *This type is available if Syn is built with the `"parsing"` feature.*
//
// TODO: change the parse module link to an intra rustdoc link, currently
// blocked on https://github.com/rust-lang/rust/issues/62830
pub struct Error {
    // Span is implemented as an index into a thread-local interner to keep the
    // size small. It is not safe to access from a different thread. We want
    // errors to be Send and Sync to play nicely with the Failure crate, so pin
    // the span we're given to its original thread and assume it is
    // Span::call_site if accessed from any other thread.
    start_span: ThreadBound<Span>,
    end_span: ThreadBound<Span>,
    message: String,
}

#[cfg(test)]
struct _Test
where
    Error: Send + Sync;

impl Error {
    /// Usually the [`ParseStream::error`] method will be used instead, which
    /// automatically uses the correct span from the current position of the
    /// parse stream.
    ///
    /// Use `Error::new` when the error needs to be triggered on some span other
    /// than where the parse stream is currently positioned.
    ///
    /// [`ParseStream::error`]: crate::parse::ParseBuffer::error
    ///
    /// # Example
    ///
    /// ```edition2018
    /// use syn::{Error, Ident, LitStr, Result, Token};
    /// use syn::parse::ParseStream;
    ///
    /// // Parses input that looks like `name = "string"` where the key must be
    /// // the identifier `name` and the value may be any string literal.
    /// // Returns the string literal.
    /// fn parse_name(input: ParseStream) -> Result<LitStr> {
    ///     let name_token: Ident = input.parse()?;
    ///     if name_token != "name" {
    ///         // Trigger an error not on the current position of the stream,
    ///         // but on the position of the unexpected identifier.
    ///         return Err(Error::new(name_token.span(), "expected `name`"));
    ///     }
    ///     input.parse::<Token![=]>()?;
    ///     let s: LitStr = input.parse()?;
    ///     Ok(s)
    /// }
    /// ```
    pub fn new<T: Display>(span: Span, message: T) -> Self {
        Error {
            start_span: ThreadBound::new(span),
            end_span: ThreadBound::new(span),
            message: message.to_string(),
        }
    }

    /// Creates an error with the specified message spanning the given syntax
    /// tree node.
    ///
    /// Unlike the `Error::new` constructor, this constructor takes an argument
    /// `tokens` which is a syntax tree node. This allows the resulting `Error`
    /// to attempt to span all tokens inside of `tokens`. While you would
    /// typically be able to use the `Spanned` trait with the above `Error::new`
    /// constructor, implementation limitations today mean that
    /// `Error::new_spanned` may provide a higher-quality error message on
    /// stable Rust.
    ///
    /// When in doubt it's recommended to stick to `Error::new` (or
    /// `ParseStream::error`)!
    #[cfg(feature = "printing")]
    pub fn new_spanned<T: ToTokens, U: Display>(tokens: T, message: U) -> Self {
        let mut iter = tokens.into_token_stream().into_iter();
        let start = iter.next().map_or_else(Span::call_site, |t| t.span());
        let end = iter.last().map_or(start, |t| t.span());
        Error {
            start_span: ThreadBound::new(start),
            end_span: ThreadBound::new(end),
            message: message.to_string(),
        }
    }

    /// The source location of the error.
    ///
    /// Spans are not thread-safe so this function returns `Span::call_site()`
    /// if called from a different thread than the one on which the `Error` was
    /// originally created.
    pub fn span(&self) -> Span {
        let start = match self.start_span.get() {
            Some(span) => *span,
            None => return Span::call_site(),
        };

        #[cfg(procmacro2_semver_exempt)]
        {
            let end = match self.end_span.get() {
                Some(span) => *span,
                None => return Span::call_site(),
            };
            start.join(end).unwrap_or(start)
        }
        #[cfg(not(procmacro2_semver_exempt))]
        {
            start
        }
    }

    /// Render the error as an invocation of [`compile_error!`].
    ///
    /// The [`parse_macro_input!`] macro provides a convenient way to invoke
    /// this method correctly in a procedural macro.
    ///
    /// [`compile_error!`]: https://doc.rust-lang.org/std/macro.compile_error.html
    pub fn to_compile_error(&self) -> TokenStream {
        let start = self
            .start_span
            .get()
            .cloned()
            .unwrap_or_else(Span::call_site);
        let end = self.end_span.get().cloned().unwrap_or_else(Span::call_site);

        // compile_error!($message)
        TokenStream::from_iter(vec![
            TokenTree::Ident(Ident::new("compile_error", start)),
            TokenTree::Punct({
                let mut punct = Punct::new('!', Spacing::Alone);
                punct.set_span(start);
                punct
            }),
            TokenTree::Group({
                let mut group = Group::new(Delimiter::Brace, {
                    TokenStream::from_iter(vec![TokenTree::Literal({
                        let mut string = Literal::string(&self.message);
                        string.set_span(end);
                        string
                    })])
                });
                group.set_span(end);
                group
            }),
        ])
    }
}

#[cfg(feature = "parsing")]
pub fn new_at<T: Display>(scope: Span, cursor: Cursor, message: T) -> Error {
    if cursor.eof() {
        Error::new(scope, format!("unexpected end of input, {}", message))
    } else {
        #[cfg(procmacro2_semver_exempt)]
        let span = private::open_span_of_group(cursor);
        #[cfg(not(procmacro2_semver_exempt))]
        let span = cursor.span();
        Error::new(span, message)
    }
}

impl Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.debug_tuple("Error").field(&self.message).finish()
    }
}

impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Clone for Error {
    fn clone(&self) -> Self {
        let start = self
            .start_span
            .get()
            .cloned()
            .unwrap_or_else(Span::call_site);
        let end = self.end_span.get().cloned().unwrap_or_else(Span::call_site);
        Error {
            start_span: ThreadBound::new(start),
            end_span: ThreadBound::new(end),
            message: self.message.clone(),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        "parse error"
    }
}

impl From<LexError> for Error {
    fn from(err: LexError) -> Self {
        Error::new(Span::call_site(), format!("{:?}", err))
    }
}

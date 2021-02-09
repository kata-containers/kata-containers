//! This crate provides the [`quote!`] macro for turning Rust syntax tree data
//! structures into tokens of source code.
//!
//! [`quote!`]: macro.quote.html
//!
//! Procedural macros in Rust receive a stream of tokens as input, execute
//! arbitrary Rust code to determine how to manipulate those tokens, and produce
//! a stream of tokens to hand back to the compiler to compile into the caller's
//! crate. Quasi-quoting is a solution to one piece of that -- producing tokens
//! to return to the compiler.
//!
//! The idea of quasi-quoting is that we write *code* that we treat as *data*.
//! Within the `quote!` macro, we can write what looks like code to our text
//! editor or IDE. We get all the benefits of the editor's brace matching,
//! syntax highlighting, indentation, and maybe autocompletion. But rather than
//! compiling that as code into the current crate, we can treat it as data, pass
//! it around, mutate it, and eventually hand it back to the compiler as tokens
//! to compile into the macro caller's crate.
//!
//! This crate is motivated by the procedural macro use case, but is a
//! general-purpose Rust quasi-quoting library and is not specific to procedural
//! macros.
//!
//! *Version requirement: Quote supports any compiler version back to Rust's
//! very first support for procedural macros in Rust 1.15.0.*
//!
//! ```toml
//! [dependencies]
//! quote = "0.6"
//! ```
//!
//! # Example
//!
//! The following quasi-quoted block of code is something you might find in [a]
//! procedural macro having to do with data structure serialization. The `#var`
//! syntax performs interpolation of runtime variables into the quoted tokens.
//! Check out the documentation of the [`quote!`] macro for more detail about
//! the syntax. See also the [`quote_spanned!`] macro which is important for
//! implementing hygienic procedural macros.
//!
//! [a]: https://serde.rs/
//! [`quote_spanned!`]: macro.quote_spanned.html
//!
//! ```edition2018
//! # use quote::quote;
//! #
//! # let generics = "";
//! # let where_clause = "";
//! # let field_ty = "";
//! # let item_ty = "";
//! # let path = "";
//! # let value = "";
//! #
//! let tokens = quote! {
//!     struct SerializeWith #generics #where_clause {
//!         value: &'a #field_ty,
//!         phantom: core::marker::PhantomData<#item_ty>,
//!     }
//!
//!     impl #generics serde::Serialize for SerializeWith #generics #where_clause {
//!         fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//!         where
//!             S: serde::Serializer,
//!         {
//!             #path(self.value, serializer)
//!         }
//!     }
//!
//!     SerializeWith {
//!         value: #value,
//!         phantom: core::marker::PhantomData::<#item_ty>,
//!     }
//! };
//! ```
//!
//! # Recursion limit
//!
//! The `quote!` macro relies on deep recursion so some large invocations may
//! fail with "recursion limit reached" when you compile. If it fails, bump up
//! the recursion limit by adding `#![recursion_limit = "128"]` to your crate.
//! An even higher limit may be necessary for especially large invocations.

// Quote types in rustdoc of other crates get linked to here.
#![doc(html_root_url = "https://docs.rs/quote/0.6.13")]

#[cfg(all(
    not(all(target_arch = "wasm32", target_os = "unknown")),
    feature = "proc-macro"
))]
extern crate proc_macro;
extern crate proc_macro2;

mod ext;
pub use ext::TokenStreamExt;

mod to_tokens;
pub use to_tokens::ToTokens;

// Not public API.
#[doc(hidden)]
#[path = "runtime.rs"]
pub mod __rt;

/// The whole point.
///
/// Performs variable interpolation against the input and produces it as
/// [`TokenStream`]. For returning tokens to the compiler in a procedural macro, use
/// `into()` to build a `TokenStream`.
///
/// [`TokenStream`]: https://docs.rs/proc-macro2/0.4/proc_macro2/struct.TokenStream.html
///
/// # Interpolation
///
/// Variable interpolation is done with `#var` (similar to `$var` in
/// `macro_rules!` macros). This grabs the `var` variable that is currently in
/// scope and inserts it in that location in the output tokens. Any type
/// implementing the [`ToTokens`] trait can be interpolated. This includes most
/// Rust primitive types as well as most of the syntax tree types from the [Syn]
/// crate.
///
/// [`ToTokens`]: trait.ToTokens.html
/// [Syn]: https://github.com/dtolnay/syn
///
/// Repetition is done using `#(...)*` or `#(...),*` again similar to
/// `macro_rules!`. This iterates through the elements of any variable
/// interpolated within the repetition and inserts a copy of the repetition body
/// for each one. The variables in an interpolation may be anything that
/// implements `IntoIterator`, including `Vec` or a pre-existing iterator.
///
/// - `#(#var)*` — no separators
/// - `#(#var),*` — the character before the asterisk is used as a separator
/// - `#( struct #var; )*` — the repetition can contain other tokens
/// - `#( #k => println!("{}", #v), )*` — even multiple interpolations
///
/// There are two limitations around interpolations in a repetition:
///
/// - Every interpolation inside of a repetition must be a distinct variable.
///   That is, `#(#a #a)*` is not allowed. Work around this by collecting `a`
///   into a vector and taking references `a1 = &a` and `a2 = &a` which you use
///   inside the repetition: `#(#a1 #a2)*`. Where possible, use meaningful names
///   that indicate the distinct role of each copy.
///
/// - Every interpolation inside of a repetition must be iterable. If we have
///   `vec` which is a vector and `ident` which is a single identifier,
///   `#(#ident #vec)*` is not allowed. Work around this by using
///   `std::iter::repeat(ident)` to produce an iterable that can be used from
///   within the repetition.
///
/// # Hygiene
///
/// Any interpolated tokens preserve the `Span` information provided by their
/// `ToTokens` implementation. Tokens that originate within the `quote!`
/// invocation are spanned with [`Span::call_site()`].
///
/// [`Span::call_site()`]: https://docs.rs/proc-macro2/0.4/proc_macro2/struct.Span.html#method.call_site
///
/// A different span can be provided through the [`quote_spanned!`] macro.
///
/// [`quote_spanned!`]: macro.quote_spanned.html
///
/// # Return type
///
/// The macro evaluates to an expression of type `proc_macro2::TokenStream`.
/// Meanwhile Rust procedural macros are expected to return the type
/// `proc_macro::TokenStream`.
///
/// The difference between the two types is that `proc_macro` types are entirely
/// specific to procedural macros and cannot ever exist in code outside of a
/// procedural macro, while `proc_macro2` types may exist anywhere including
/// tests and non-macro code like main.rs and build.rs. This is why even the
/// procedural macro ecosystem is largely built around `proc_macro2`, because
/// that ensures the libraries are unit testable and accessible in non-macro
/// contexts.
///
/// There is a [`From`]-conversion in both directions so returning the output of
/// `quote!` from a procedural macro usually looks like `tokens.into()` or
/// `proc_macro::TokenStream::from(tokens)`.
///
/// [`From`]: https://doc.rust-lang.org/std/convert/trait.From.html
///
/// # Examples
///
/// ## Procedural macro
///
/// The structure of a basic procedural macro is as follows. Refer to the [Syn]
/// crate for further useful guidance on using `quote!` as part of a procedural
/// macro.
///
/// [Syn]: https://github.com/dtolnay/syn
///
/// ```edition2018
/// # #[cfg(any())]
/// extern crate proc_macro;
/// # use proc_macro2 as proc_macro;
///
/// use proc_macro::TokenStream;
/// use quote::quote;
///
/// # const IGNORE_TOKENS: &'static str = stringify! {
/// #[proc_macro_derive(HeapSize)]
/// # };
/// pub fn derive_heap_size(input: TokenStream) -> TokenStream {
///     // Parse the input and figure out what implementation to generate...
///     # const IGNORE_TOKENS: &'static str = stringify! {
///     let name = /* ... */;
///     let expr = /* ... */;
///     # };
///     #
///     # let name = 0;
///     # let expr = 0;
///
///     let expanded = quote! {
///         // The generated impl.
///         impl heapsize::HeapSize for #name {
///             fn heap_size_of_children(&self) -> usize {
///                 #expr
///             }
///         }
///     };
///
///     // Hand the output tokens back to the compiler.
///     TokenStream::from(expanded)
/// }
/// ```
///
/// ## Combining quoted fragments
///
/// Usually you don't end up constructing an entire final `TokenStream` in one
/// piece. Different parts may come from different helper functions. The tokens
/// produced by `quote!` themselves implement `ToTokens` and so can be
/// interpolated into later `quote!` invocations to build up a final result.
///
/// ```edition2018
/// # use quote::quote;
/// #
/// let type_definition = quote! {...};
/// let methods = quote! {...};
///
/// let tokens = quote! {
///     #type_definition
///     #methods
/// };
/// ```
///
/// ## Constructing identifiers
///
/// Suppose we have an identifier `ident` which came from somewhere in a macro
/// input and we need to modify it in some way for the macro output. Let's
/// consider prepending the identifier with an underscore.
///
/// Simply interpolating the identifier next to an underscore will not have the
/// behavior of concatenating them. The underscore and the identifier will
/// continue to be two separate tokens as if you had written `_ x`.
///
/// ```edition2018
/// # use proc_macro2::{self as syn, Span};
/// # use quote::quote;
/// #
/// # let ident = syn::Ident::new("i", Span::call_site());
/// #
/// // incorrect
/// quote! {
///     let mut _#ident = 0;
/// }
/// # ;
/// ```
///
/// The solution is to perform token-level manipulations using the APIs provided
/// by Syn and proc-macro2.
///
/// ```edition2018
/// # use proc_macro2::{self as syn, Span};
/// # use quote::quote;
/// #
/// # let ident = syn::Ident::new("i", Span::call_site());
/// #
/// let concatenated = format!("_{}", ident);
/// let varname = syn::Ident::new(&concatenated, ident.span());
/// quote! {
///     let mut #varname = 0;
/// }
/// # ;
/// ```
///
/// ## Making method calls
///
/// Let's say our macro requires some type specified in the macro input to have
/// a constructor called `new`. We have the type in a variable called
/// `field_type` of type `syn::Type` and want to invoke the constructor.
///
/// ```edition2018
/// # use quote::quote;
/// #
/// # let field_type = quote!(...);
/// #
/// // incorrect
/// quote! {
///     let value = #field_type::new();
/// }
/// # ;
/// ```
///
/// This works only sometimes. If `field_type` is `String`, the expanded code
/// contains `String::new()` which is fine. But if `field_type` is something
/// like `Vec<i32>` then the expanded code is `Vec<i32>::new()` which is invalid
/// syntax. Ordinarily in handwritten Rust we would write `Vec::<i32>::new()`
/// but for macros often the following is more convenient.
///
/// ```edition2018
/// # use quote::quote;
/// #
/// # let field_type = quote!(...);
/// #
/// quote! {
///     let value = <#field_type>::new();
/// }
/// # ;
/// ```
///
/// This expands to `<Vec<i32>>::new()` which behaves correctly.
///
/// A similar pattern is appropriate for trait methods.
///
/// ```edition2018
/// # use quote::quote;
/// #
/// # let field_type = quote!(...);
/// #
/// quote! {
///     let value = <#field_type as core::default::Default>::default();
/// }
/// # ;
/// ```
#[macro_export(local_inner_macros)]
macro_rules! quote {
    ($($tt:tt)*) => {
        quote_spanned!($crate::__rt::Span::call_site()=> $($tt)*)
    };
}

/// Same as `quote!`, but applies a given span to all tokens originating within
/// the macro invocation.
///
/// # Syntax
///
/// A span expression of type [`Span`], followed by `=>`, followed by the tokens
/// to quote. The span expression should be brief -- use a variable for anything
/// more than a few characters. There should be no space before the `=>` token.
///
/// [`Span`]: https://docs.rs/proc-macro2/0.4/proc_macro2/struct.Span.html
///
/// ```edition2018
/// # use proc_macro2::Span;
/// # use quote::quote_spanned;
/// #
/// # const IGNORE_TOKENS: &'static str = stringify! {
/// let span = /* ... */;
/// # };
/// # let span = Span::call_site();
/// # let init = 0;
///
/// // On one line, use parentheses.
/// let tokens = quote_spanned!(span=> Box::into_raw(Box::new(#init)));
///
/// // On multiple lines, place the span at the top and use braces.
/// let tokens = quote_spanned! {span=>
///     Box::into_raw(Box::new(#init))
/// };
/// ```
///
/// The lack of space before the `=>` should look jarring to Rust programmers
/// and this is intentional. The formatting is designed to be visibly
/// off-balance and draw the eye a particular way, due to the span expression
/// being evaluated in the context of the procedural macro and the remaining
/// tokens being evaluated in the generated code.
///
/// # Hygiene
///
/// Any interpolated tokens preserve the `Span` information provided by their
/// `ToTokens` implementation. Tokens that originate within the `quote_spanned!`
/// invocation are spanned with the given span argument.
///
/// # Example
///
/// The following procedural macro code uses `quote_spanned!` to assert that a
/// particular Rust type implements the [`Sync`] trait so that references can be
/// safely shared between threads.
///
/// [`Sync`]: https://doc.rust-lang.org/std/marker/trait.Sync.html
///
/// ```edition2018
/// # use quote::{quote_spanned, TokenStreamExt, ToTokens};
/// # use proc_macro2::{Span, TokenStream};
/// #
/// # struct Type;
/// #
/// # impl Type {
/// #     fn span(&self) -> Span {
/// #         Span::call_site()
/// #     }
/// # }
/// #
/// # impl ToTokens for Type {
/// #     fn to_tokens(&self, _tokens: &mut TokenStream) {}
/// # }
/// #
/// # let ty = Type;
/// # let call_site = Span::call_site();
/// #
/// let ty_span = ty.span();
/// let assert_sync = quote_spanned! {ty_span=>
///     struct _AssertSync where #ty: Sync;
/// };
/// ```
///
/// If the assertion fails, the user will see an error like the following. The
/// input span of their type is hightlighted in the error.
///
/// ```text
/// error[E0277]: the trait bound `*const (): std::marker::Sync` is not satisfied
///   --> src/main.rs:10:21
///    |
/// 10 |     static ref PTR: *const () = &();
///    |                     ^^^^^^^^^ `*const ()` cannot be shared between threads safely
/// ```
///
/// In this example it is important for the where-clause to be spanned with the
/// line/column information of the user's input type so that error messages are
/// placed appropriately by the compiler. But it is also incredibly important
/// that `Sync` resolves at the macro definition site and not the macro call
/// site. If we resolve `Sync` at the same span that the user's type is going to
/// be resolved, then they could bypass our check by defining their own trait
/// named `Sync` that is implemented for their type.
#[macro_export(local_inner_macros)]
macro_rules! quote_spanned {
    ($span:expr=> $($tt:tt)*) => {{
        let mut _s = $crate::__rt::TokenStream::new();
        let _span = $span;
        quote_each_token!(_s _span $($tt)*);
        _s
    }};
}

// Extract the names of all #metavariables and pass them to the $finish macro.
//
// in:   pounded_var_names!(then () a #b c #( #d )* #e)
// out:  then!(() b d e)
#[macro_export(local_inner_macros)]
#[doc(hidden)]
macro_rules! pounded_var_names {
    ($finish:ident ($($found:ident)*) # ( $($inner:tt)* ) $($rest:tt)*) => {
        pounded_var_names!($finish ($($found)*) $($inner)* $($rest)*)
    };

    ($finish:ident ($($found:ident)*) # [ $($inner:tt)* ] $($rest:tt)*) => {
        pounded_var_names!($finish ($($found)*) $($inner)* $($rest)*)
    };

    ($finish:ident ($($found:ident)*) # { $($inner:tt)* } $($rest:tt)*) => {
        pounded_var_names!($finish ($($found)*) $($inner)* $($rest)*)
    };

    ($finish:ident ($($found:ident)*) # $first:ident $($rest:tt)*) => {
        pounded_var_names!($finish ($($found)* $first) $($rest)*)
    };

    ($finish:ident ($($found:ident)*) ( $($inner:tt)* ) $($rest:tt)*) => {
        pounded_var_names!($finish ($($found)*) $($inner)* $($rest)*)
    };

    ($finish:ident ($($found:ident)*) [ $($inner:tt)* ] $($rest:tt)*) => {
        pounded_var_names!($finish ($($found)*) $($inner)* $($rest)*)
    };

    ($finish:ident ($($found:ident)*) { $($inner:tt)* } $($rest:tt)*) => {
        pounded_var_names!($finish ($($found)*) $($inner)* $($rest)*)
    };

    ($finish:ident ($($found:ident)*) $ignore:tt $($rest:tt)*) => {
        pounded_var_names!($finish ($($found)*) $($rest)*)
    };

    ($finish:ident ($($found:ident)*)) => {
        $finish!(() $($found)*)
    };
}

// in:   nested_tuples_pat!(() a b c d e)
// out:  ((((a b) c) d) e)
//
// in:   nested_tuples_pat!(() a)
// out:  a
#[macro_export(local_inner_macros)]
#[doc(hidden)]
macro_rules! nested_tuples_pat {
    (()) => {
        &()
    };

    (() $first:ident $($rest:ident)*) => {
        nested_tuples_pat!(($first) $($rest)*)
    };

    (($pat:pat) $first:ident $($rest:ident)*) => {
        nested_tuples_pat!((($pat, $first)) $($rest)*)
    };

    (($done:pat)) => {
        $done
    };
}

// in:   multi_zip_expr!(() a b c d e)
// out:  a.into_iter().zip(b).zip(c).zip(d).zip(e)
//
// in:   multi_zip_iter!(() a)
// out:  a
#[macro_export(local_inner_macros)]
#[doc(hidden)]
macro_rules! multi_zip_expr {
    (()) => {
        &[]
    };

    (() $single:ident) => {
        $single
    };

    (() $first:ident $($rest:ident)*) => {
        multi_zip_expr!(($first.into_iter()) $($rest)*)
    };

    (($zips:expr) $first:ident $($rest:ident)*) => {
        multi_zip_expr!(($zips.zip($first)) $($rest)*)
    };

    (($done:expr)) => {
        $done
    };
}

#[macro_export(local_inner_macros)]
#[doc(hidden)]
macro_rules! quote_each_token {
    ($tokens:ident $span:ident) => {};

    ($tokens:ident $span:ident # ! $($rest:tt)*) => {
        quote_each_token!($tokens $span #);
        quote_each_token!($tokens $span !);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident # ( $($inner:tt)* ) * $($rest:tt)*) => {
        for pounded_var_names!(nested_tuples_pat () $($inner)*)
        in pounded_var_names!(multi_zip_expr () $($inner)*) {
            quote_each_token!($tokens $span $($inner)*);
        }
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident # ( $($inner:tt)* ) $sep:tt * $($rest:tt)*) => {
        for (_i, pounded_var_names!(nested_tuples_pat () $($inner)*))
        in pounded_var_names!(multi_zip_expr () $($inner)*).into_iter().enumerate() {
            if _i > 0 {
                quote_each_token!($tokens $span $sep);
            }
            quote_each_token!($tokens $span $($inner)*);
        }
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident # [ $($inner:tt)* ] $($rest:tt)*) => {
        quote_each_token!($tokens $span #);
        $tokens.extend({
            let mut g = $crate::__rt::Group::new(
                $crate::__rt::Delimiter::Bracket,
                quote_spanned!($span=> $($inner)*),
            );
            g.set_span($span);
            Some($crate::__rt::TokenTree::from(g))
        });
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident # $first:ident $($rest:tt)*) => {
        $crate::ToTokens::to_tokens(&$first, &mut $tokens);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident ( $($first:tt)* ) $($rest:tt)*) => {
        $tokens.extend({
            let mut g = $crate::__rt::Group::new(
                $crate::__rt::Delimiter::Parenthesis,
                quote_spanned!($span=> $($first)*),
            );
            g.set_span($span);
            Some($crate::__rt::TokenTree::from(g))
        });
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident [ $($first:tt)* ] $($rest:tt)*) => {
        $tokens.extend({
            let mut g = $crate::__rt::Group::new(
                $crate::__rt::Delimiter::Bracket,
                quote_spanned!($span=> $($first)*),
            );
            g.set_span($span);
            Some($crate::__rt::TokenTree::from(g))
        });
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident { $($first:tt)* } $($rest:tt)*) => {
        $tokens.extend({
            let mut g = $crate::__rt::Group::new(
                $crate::__rt::Delimiter::Brace,
                quote_spanned!($span=> $($first)*),
            );
            g.set_span($span);
            Some($crate::__rt::TokenTree::from(g))
        });
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident + $($rest:tt)*) => {
        $crate::__rt::push_add(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident += $($rest:tt)*) => {
        $crate::__rt::push_add_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident & $($rest:tt)*) => {
        $crate::__rt::push_and(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident && $($rest:tt)*) => {
        $crate::__rt::push_and_and(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident &= $($rest:tt)*) => {
        $crate::__rt::push_and_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident @ $($rest:tt)*) => {
        $crate::__rt::push_at(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident ! $($rest:tt)*) => {
        $crate::__rt::push_bang(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident ^ $($rest:tt)*) => {
        $crate::__rt::push_caret(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident ^= $($rest:tt)*) => {
        $crate::__rt::push_caret_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident : $($rest:tt)*) => {
        $crate::__rt::push_colon(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident :: $($rest:tt)*) => {
        $crate::__rt::push_colon2(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident , $($rest:tt)*) => {
        $crate::__rt::push_comma(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident / $($rest:tt)*) => {
        $crate::__rt::push_div(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident /= $($rest:tt)*) => {
        $crate::__rt::push_div_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident . $($rest:tt)*) => {
        $crate::__rt::push_dot(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident .. $($rest:tt)*) => {
        $crate::__rt::push_dot2(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident ... $($rest:tt)*) => {
        $crate::__rt::push_dot3(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident ..= $($rest:tt)*) => {
        $crate::__rt::push_dot_dot_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident = $($rest:tt)*) => {
        $crate::__rt::push_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident == $($rest:tt)*) => {
        $crate::__rt::push_eq_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident >= $($rest:tt)*) => {
        $crate::__rt::push_ge(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident > $($rest:tt)*) => {
        $crate::__rt::push_gt(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident <= $($rest:tt)*) => {
        $crate::__rt::push_le(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident < $($rest:tt)*) => {
        $crate::__rt::push_lt(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident *= $($rest:tt)*) => {
        $crate::__rt::push_mul_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident != $($rest:tt)*) => {
        $crate::__rt::push_ne(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident | $($rest:tt)*) => {
        $crate::__rt::push_or(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident |= $($rest:tt)*) => {
        $crate::__rt::push_or_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident || $($rest:tt)*) => {
        $crate::__rt::push_or_or(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident # $($rest:tt)*) => {
        $crate::__rt::push_pound(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident ? $($rest:tt)*) => {
        $crate::__rt::push_question(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident -> $($rest:tt)*) => {
        $crate::__rt::push_rarrow(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident <- $($rest:tt)*) => {
        $crate::__rt::push_larrow(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident % $($rest:tt)*) => {
        $crate::__rt::push_rem(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident %= $($rest:tt)*) => {
        $crate::__rt::push_rem_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident => $($rest:tt)*) => {
        $crate::__rt::push_fat_arrow(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident ; $($rest:tt)*) => {
        $crate::__rt::push_semi(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident << $($rest:tt)*) => {
        $crate::__rt::push_shl(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident <<= $($rest:tt)*) => {
        $crate::__rt::push_shl_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident >> $($rest:tt)*) => {
        $crate::__rt::push_shr(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident >>= $($rest:tt)*) => {
        $crate::__rt::push_shr_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident * $($rest:tt)*) => {
        $crate::__rt::push_star(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident - $($rest:tt)*) => {
        $crate::__rt::push_sub(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident -= $($rest:tt)*) => {
        $crate::__rt::push_sub_eq(&mut $tokens, $span);
        quote_each_token!($tokens $span $($rest)*);
    };

    ($tokens:ident $span:ident $first:tt $($rest:tt)*) => {
        $crate::__rt::parse(&mut $tokens, $span, quote_stringify!($first));
        quote_each_token!($tokens $span $($rest)*);
    };
}

// Unhygienically invoke whatever `stringify` the caller has in scope i.e. not a
// local macro. The macros marked `local_inner_macros` above cannot invoke
// `stringify` directly.
#[macro_export]
#[doc(hidden)]
macro_rules! quote_stringify {
    ($tt:tt) => {
        stringify!($tt)
    };
}

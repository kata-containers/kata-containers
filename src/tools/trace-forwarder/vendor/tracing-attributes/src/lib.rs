//! A procedural macro attribute for instrumenting functions with [`tracing`].
//!
//! [`tracing`] is a framework for instrumenting Rust programs to collect
//! structured, event-based diagnostic information. This crate provides the
//! [`#[instrument]`][instrument] procedural macro attribute.
//!
//! Note that this macro is also re-exported by the main `tracing` crate.
//!
//! *Compiler support: [requires `rustc` 1.42+][msrv]*
//!
//! [msrv]: #supported-rust-versions
//!
//! ## Usage
//!
//! First, add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tracing-attributes = "0.1.18"
//! ```
//!
//! The [`#[instrument]`][instrument] attribute can now be added to a function
//! to automatically create and enter `tracing` [span] when that function is
//! called. For example:
//!
//! ```
//! use tracing_attributes::instrument;
//!
//! #[instrument]
//! pub fn my_function(my_arg: usize) {
//!     // ...
//! }
//!
//! # fn main() {}
//! ```
//!
//! [`tracing`]: https://crates.io/crates/tracing
//! [span]: https://docs.rs/tracing/latest/tracing/span/index.html
//! [instrument]: attr.instrument.html
//!
//! ## Supported Rust Versions
//!
//! Tracing is built against the latest stable release. The minimum supported
//! version is 1.42. The current Tracing version is not guaranteed to build on
//! Rust versions earlier than the minimum supported version.
//!
//! Tracing follows the same compiler support policies as the rest of the Tokio
//! project. The current stable Rust compiler and the three most recent minor
//! versions before it will always be supported. For example, if the current
//! stable compiler version is 1.45, the minimum supported version will not be
//! increased past 1.42, three minor versions prior. Increasing the minimum
//! supported compiler version is not considered a semver breaking change as
//! long as doing so complies with this policy.
//!
#![doc(html_root_url = "https://docs.rs/tracing-attributes/0.1.18")]
#![doc(
    html_logo_url = "https://raw.githubusercontent.com/tokio-rs/tracing/master/assets/logo-type.png",
    issue_tracker_base_url = "https://github.com/tokio-rs/tracing/issues/"
)]
#![cfg_attr(docsrs, deny(rustdoc::broken_intra_doc_links))]
#![warn(
    missing_debug_implementations,
    missing_docs,
    rust_2018_idioms,
    unreachable_pub,
    bad_style,
    const_err,
    dead_code,
    improper_ctypes,
    non_shorthand_field_patterns,
    no_mangle_generic_items,
    overflowing_literals,
    path_statements,
    patterns_in_fns_without_body,
    private_in_public,
    unconditional_recursion,
    unused_allocation,
    unused_comparisons,
    unused_parens,
    while_true
)]
// TODO: once `tracing` bumps its MSRV to 1.42, remove this allow.
#![allow(unused)]
extern crate proc_macro;

use std::collections::{HashMap, HashSet};
use std::iter;

use proc_macro2::TokenStream;
use quote::{quote, quote_spanned, ToTokens, TokenStreamExt as _};
use syn::ext::IdentExt as _;
use syn::parse::{Parse, ParseStream};
use syn::{
    punctuated::Punctuated, spanned::Spanned, Block, Expr, ExprAsync, ExprCall, FieldPat, FnArg,
    Ident, Item, ItemFn, LitInt, LitStr, Pat, PatIdent, PatReference, PatStruct, PatTuple,
    PatTupleStruct, PatType, Path, Signature, Stmt, Token, TypePath,
};
/// Instruments a function to create and enter a `tracing` [span] every time
/// the function is called.
///
/// Unless overriden, a span with `info` level will be generated.
/// The generated span's name will be the name of the function.
/// By default, all arguments to the function are included as fields on the
/// span. Arguments that are `tracing` [primitive types] implementing the
/// [`Value` trait] will be recorded as fields of that type. Types which do
/// not implement `Value` will be recorded using [`std::fmt::Debug`].
///
/// [primitive types]: https://docs.rs/tracing/latest/tracing/field/trait.Value.html#foreign-impls
/// [`Value` trait]: https://docs.rs/tracing/latest/tracing/field/trait.Value.html.
///
/// # Overriding Span Attributes
///
/// To change the [name] of the generated span, add a `name` argument to the
/// `#[instrument]` macro, followed by an equals sign and a string literal. For
/// example:
///
/// ```
/// # use tracing_attributes::instrument;
///
/// // The generated span's name will be "my_span" rather than "my_function".
/// #[instrument(name = "my_span")]
/// pub fn my_function() {
///     // ... do something incredibly interesting and important ...
/// }
/// ```
///
/// To override the [target] of the generated span, add a `target` argument to
/// the `#[instrument]` macro, followed by an equals sign and a string literal
/// for the new target. The [module path] is still recorded separately. For
/// example:
///
/// ```
/// pub mod my_module {
///     # use tracing_attributes::instrument;
///     // The generated span's target will be "my_crate::some_special_target",
///     // rather than "my_crate::my_module".
///     #[instrument(target = "my_crate::some_special_target")]
///     pub fn my_function() {
///         // ... all kinds of neat code in here ...
///     }
/// }
/// ```
///
/// Finally, to override the [level] of the generated span, add a `level`
/// argument, followed by an equals sign and a string literal with the name of
/// the desired level. Level names are not case sensitive. For example:
///
/// ```
/// # use tracing_attributes::instrument;
/// // The span's level will be TRACE rather than INFO.
/// #[instrument(level = "trace")]
/// pub fn my_function() {
///     // ... I have written a truly marvelous implementation of this function,
///     // which this example is too narrow to contain ...
/// }
/// ```
///
/// # Skipping Fields
///
/// To skip recording one or more arguments to a function or method, pass
/// the argument's name inside the `skip()` argument on the `#[instrument]`
/// macro. This can be used when an argument to an instrumented function does
/// not implement [`fmt::Debug`], or to exclude an argument with a verbose or
/// costly `Debug` implementation. Note that:
///
/// - multiple argument names can be passed to `skip`.
/// - arguments passed to `skip` do _not_ need to implement `fmt::Debug`.
///
/// You can also use `skip_all` to skip all arguments.
///
/// ## Examples
///
/// ```
/// # use tracing_attributes::instrument;
/// # use std::collections::HashMap;
/// // This type doesn't implement `fmt::Debug`!
/// struct NonDebug;
///
/// // `arg` will be recorded, while `non_debug` will not.
/// #[instrument(skip(non_debug))]
/// fn my_function(arg: usize, non_debug: NonDebug) {
///     // ...
/// }
///
/// // These arguments are huge
/// #[instrument(skip_all)]
/// fn my_big_data_function(large: Vec<u8>, also_large: HashMap<String, String>) {
///     // ...
/// }
/// ```
///
/// Skipping the `self` parameter:
///
/// ```
/// # use tracing_attributes::instrument;
/// #[derive(Debug)]
/// struct MyType {
///    data: Vec<u8>, // Suppose this buffer is often quite long...
/// }
///
/// impl MyType {
///     // Suppose we don't want to print an entire kilobyte of `data`
///     // every time this is called...
///     #[instrument(skip(self))]
///     pub fn my_method(&mut self, an_interesting_argument: usize) {
///          // ... do something (hopefully, using all that `data`!)
///     }
/// }
/// ```
///
/// # Adding Fields
///
/// Additional fields (key-value pairs with arbitrary data) may be added to the
/// generated span using the `fields` argument on the `#[instrument]` macro. Any
/// Rust expression can be used as a field value in this manner. These
/// expressions will be evaluated at the beginning of the function's body, so
/// arguments to the function may be used in these expressions. Field names may
/// also be specified *without* values. Doing so will result in an [empty field]
/// whose value may be recorded later within the function body.
///
/// This supports the same [field syntax] as the `span!` and `event!` macros.
///
/// Note that overlap between the names of fields and (non-skipped) arguments
/// will result in a compile error.
///
/// ## Examples
///
/// Adding a new field based on the value of an argument:
///
/// ```
/// # use tracing_attributes::instrument;
///
/// // This will record a field named "i" with the value of `i` *and* a field
/// // named "next" with the value of `i` + 1.
/// #[instrument(fields(next = i + 1))]
/// pub fn my_function(i: usize) {
///     // ...
/// }
/// ```
///
/// Recording specific properties of a struct as their own fields:
///
/// ```
/// # mod http {
/// #   pub struct Error;
/// #   pub struct Response<B> { pub(super) _b: std::marker::PhantomData<B> }
/// #   pub struct Request<B> { _b: B }
/// #   impl<B> std::fmt::Debug for Request<B> {
/// #       fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
/// #           f.pad("request")
/// #       }
/// #   }
/// #   impl<B> Request<B> {
/// #       pub fn uri(&self) -> &str { "fake" }
/// #       pub fn method(&self) -> &str { "GET" }
/// #   }
/// # }
/// # use tracing_attributes::instrument;
///
/// // This will record the request's URI and HTTP method as their own separate
/// // fields.
/// #[instrument(fields(http.uri = req.uri(), http.method = req.method()))]
/// pub fn handle_request<B>(req: http::Request<B>) -> http::Response<B> {
///     // ... handle the request ...
///     # http::Response { _b: std::marker::PhantomData }
/// }
/// ```
///
/// This can be used in conjunction with `skip` or `skip_all` to record only
/// some fields of a struct:
/// ```
/// # use tracing_attributes::instrument;
/// // Remember the struct with the very large `data` field from the earlier
/// // example? Now it also has a `name`, which we might want to include in
/// // our span.
/// #[derive(Debug)]
/// struct MyType {
///    name: &'static str,
///    data: Vec<u8>,
/// }
///
/// impl MyType {
///     // This will skip the `data` field, but will include `self.name`,
///     // formatted using `fmt::Display`.
///     #[instrument(skip(self), fields(self.name = %self.name))]
///     pub fn my_method(&mut self, an_interesting_argument: usize) {
///          // ... do something (hopefully, using all that `data`!)
///     }
/// }
/// ```
///
/// Adding an empty field to be recorded later:
///
/// ```
/// # use tracing_attributes::instrument;
///
/// // This function does a very interesting and important mathematical calculation.
/// // Suppose we want to record both the inputs to the calculation *and* its result...
/// #[instrument(fields(result))]
/// pub fn do_calculation(input_1: usize, input_2: usize) -> usize {
///     // Rerform the calculation.
///     let result = input_1 + input_2;
///
///     // Record the result as part of the current span.
///     tracing::Span::current().record("result", &result);
///
///     // Now, the result will also be included on this event!
///     tracing::info!("calculation complete!");
///
///     // ... etc ...
///     # 0
/// }
/// ```
///
/// # Examples
///
/// Instrumenting a function:
///
/// ```
/// # use tracing_attributes::instrument;
/// #[instrument]
/// pub fn my_function(my_arg: usize) {
///     // This event will be recorded inside a span named `my_function` with the
///     // field `my_arg`.
///     tracing::info!("inside my_function!");
///     // ...
/// }
/// ```
/// Setting the level for the generated span:
/// ```
/// # use tracing_attributes::instrument;
/// #[instrument(level = "debug")]
/// pub fn my_function() {
///     // ...
/// }
/// ```
/// Overriding the generated span's name:
/// ```
/// # use tracing_attributes::instrument;
/// #[instrument(name = "my_name")]
/// pub fn my_function() {
///     // ...
/// }
/// ```
/// Overriding the generated span's target:
/// ```
/// # use tracing_attributes::instrument;
/// #[instrument(target = "my_target")]
/// pub fn my_function() {
///     // ...
/// }
/// ```
///
/// To skip recording an argument, pass the argument's name to the `skip`:
///
/// ```
/// # use tracing_attributes::instrument;
/// struct NonDebug;
///
/// #[instrument(skip(non_debug))]
/// fn my_function(arg: usize, non_debug: NonDebug) {
///     // ...
/// }
/// ```
///
/// To add an additional context to the span, pass key-value pairs to `fields`:
///
/// ```
/// # use tracing_attributes::instrument;
/// #[instrument(fields(foo="bar", id=1, show=true))]
/// fn my_function(arg: usize) {
///     // ...
/// }
/// ```
///
/// If the function returns a `Result<T, E>` and `E` implements `std::fmt::Display`, you can add
/// `err` to emit error events when the function returns `Err`:
///
/// ```
/// # use tracing_attributes::instrument;
/// #[instrument(err)]
/// fn my_function(arg: usize) -> Result<(), std::io::Error> {
///     Ok(())
/// }
/// ```
///
/// `async fn`s may also be instrumented:
///
/// ```
/// # use tracing_attributes::instrument;
/// #[instrument]
/// pub async fn my_function() -> Result<(), ()> {
///     // ...
///     # Ok(())
/// }
/// ```
///
/// It also works with [async-trait](https://crates.io/crates/async-trait)
/// (a crate that allows defining async functions in traits,
/// something not currently possible in Rust),
/// and hopefully most libraries that exhibit similar behaviors:
///
/// ```
/// # use tracing::instrument;
/// use async_trait::async_trait;
///
/// #[async_trait]
/// pub trait Foo {
///     async fn foo(&self, arg: usize);
/// }
///
/// #[derive(Debug)]
/// struct FooImpl(usize);
///
/// #[async_trait]
/// impl Foo for FooImpl {
///     #[instrument(fields(value = self.0, tmp = std::any::type_name::<Self>()))]
///     async fn foo(&self, arg: usize) {}
/// }
/// ```
///
/// Note than on `async-trait` <= 0.1.43, references to the `Self`
/// type inside the `fields` argument were only allowed when the instrumented
/// function is a method (i.e., the function receives `self` as an argument).
/// For example, this *used to not work* because the instrument function
/// didn't receive `self`:
/// ```
/// # use tracing::instrument;
/// use async_trait::async_trait;
///
/// #[async_trait]
/// pub trait Bar {
///     async fn bar();
/// }
///
/// #[derive(Debug)]
/// struct BarImpl(usize);
///
/// #[async_trait]
/// impl Bar for BarImpl {
///     #[instrument(fields(tmp = std::any::type_name::<Self>()))]
///     async fn bar() {}
/// }
/// ```
/// Instead, you should manually rewrite any `Self` types as the type for
/// which you implement the trait: `#[instrument(fields(tmp = std::any::type_name::<Bar>()))]`
/// (or maybe you can just bump `async-trait`).
///
/// [span]: https://docs.rs/tracing/latest/tracing/span/index.html
/// [name]: https://docs.rs/tracing/latest/tracing/struct.Metadata.html#method.name
/// [target]: https://docs.rs/tracing/latest/tracing/struct.Metadata.html#method.target
/// [level]: https://docs.rs/tracing/latest/tracing/struct.Level.html
/// [module path]: https://docs.rs/tracing/latest/tracing/struct.Metadata.html#method.module_path
/// [`INFO`]: https://docs.rs/tracing/latest/tracing/struct.Level.html#associatedconstant.INFO
/// [empty field]: https://docs.rs/tracing/latest/tracing/field/struct.Empty.html
/// [field syntax]: https://docs.rs/tracing/latest/tracing/#recording-fields
/// [`fmt::Debug`]: https://doc.rust-lang.org/std/fmt/trait.Debug.html
#[proc_macro_attribute]
pub fn instrument(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as ItemFn);
    let args = syn::parse_macro_input!(args as InstrumentArgs);

    let instrumented_function_name = input.sig.ident.to_string();

    // check for async_trait-like patterns in the block, and instrument
    // the future instead of the wrapper
    if let Some(internal_fun) = get_async_trait_info(&input.block, input.sig.asyncness.is_some()) {
        // let's rewrite some statements!
        let mut out_stmts: Vec<TokenStream> = input
            .block
            .stmts
            .iter()
            .map(|stmt| stmt.to_token_stream())
            .collect();

        if let Some((iter, _stmt)) = input
            .block
            .stmts
            .iter()
            .enumerate()
            .find(|(_iter, stmt)| *stmt == internal_fun.source_stmt)
        {
            // instrument the future by rewriting the corresponding statement
            out_stmts[iter] = match internal_fun.kind {
                // async-trait <= 0.1.43
                AsyncTraitKind::Function(fun) => gen_function(
                    fun,
                    args,
                    instrumented_function_name.as_str(),
                    internal_fun.self_type.as_ref(),
                ),
                // async-trait >= 0.1.44
                AsyncTraitKind::Async(async_expr) => {
                    let instrumented_block = gen_block(
                        &async_expr.block,
                        &input.sig.inputs,
                        true,
                        args,
                        instrumented_function_name.as_str(),
                        None,
                    );
                    let async_attrs = &async_expr.attrs;
                    quote! {
                        Box::pin(#(#async_attrs) * async move { #instrumented_block })
                    }
                }
            };
        }

        let vis = &input.vis;
        let sig = &input.sig;
        let attrs = &input.attrs;
        quote!(
            #(#attrs) *
            #vis #sig {
                #(#out_stmts) *
            }
        )
        .into()
    } else {
        gen_function(&input, args, instrumented_function_name.as_str(), None).into()
    }
}

/// Given an existing function, generate an instrumented version of that function
fn gen_function(
    input: &ItemFn,
    args: InstrumentArgs,
    instrumented_function_name: &str,
    self_type: Option<&syn::TypePath>,
) -> proc_macro2::TokenStream {
    // these are needed ahead of time, as ItemFn contains the function body _and_
    // isn't representable inside a quote!/quote_spanned! macro
    // (Syn's ToTokens isn't implemented for ItemFn)
    let ItemFn {
        attrs,
        vis,
        block,
        sig,
        ..
    } = input;

    let Signature {
        output: return_type,
        inputs: params,
        unsafety,
        asyncness,
        constness,
        abi,
        ident,
        generics:
            syn::Generics {
                params: gen_params,
                where_clause,
                ..
            },
        ..
    } = sig;

    let warnings = args.warnings();

    let body = gen_block(
        block,
        params,
        asyncness.is_some(),
        args,
        instrumented_function_name,
        self_type,
    );

    quote!(
        #(#attrs) *
        #vis #constness #unsafety #asyncness #abi fn #ident<#gen_params>(#params) #return_type
        #where_clause
        {
            #warnings
            #body
        }
    )
}

/// Instrument a block
fn gen_block(
    block: &Block,
    params: &Punctuated<FnArg, Token![,]>,
    async_context: bool,
    mut args: InstrumentArgs,
    instrumented_function_name: &str,
    self_type: Option<&syn::TypePath>,
) -> proc_macro2::TokenStream {
    let err = args.err;

    // generate the span's name
    let span_name = args
        // did the user override the span's name?
        .name
        .as_ref()
        .map(|name| quote!(#name))
        .unwrap_or_else(|| quote!(#instrumented_function_name));

    let level = args.level();

    // generate this inside a closure, so we can return early on errors.
    let span = (|| {
        // Pull out the arguments-to-be-skipped first, so we can filter results
        // below.
        let param_names: Vec<(Ident, (Ident, RecordType))> = params
            .clone()
            .into_iter()
            .flat_map(|param| match param {
                FnArg::Typed(PatType { pat, ty, .. }) => {
                    param_names(*pat, RecordType::parse_from_ty(&*ty))
                }
                FnArg::Receiver(_) => Box::new(iter::once((
                    Ident::new("self", param.span()),
                    RecordType::Debug,
                ))),
            })
            // Little dance with new (user-exposed) names and old (internal)
            // names of identifiers. That way, we could do the following
            // even though async_trait (<=0.1.43) rewrites "self" as "_self":
            // ```
            // #[async_trait]
            // impl Foo for FooImpl {
            //     #[instrument(skip(self))]
            //     async fn foo(&self, v: usize) {}
            // }
            // ```
            .map(|(x, record_type)| {
                // if we are inside a function generated by async-trait <=0.1.43, we need to
                // take care to rewrite "_self" as "self" for 'user convenience'
                if self_type.is_some() && x == "_self" {
                    (Ident::new("self", x.span()), (x, record_type))
                } else {
                    (x.clone(), (x, record_type))
                }
            })
            .collect();

        for skip in &args.skips {
            if !param_names.iter().map(|(user, _)| user).any(|y| y == skip) {
                return quote_spanned! {skip.span()=>
                    compile_error!("attempting to skip non-existent parameter")
                };
            }
        }

        let target = args.target();

        // filter out skipped fields
        let quoted_fields: Vec<_> = param_names
            .iter()
            .filter(|(param, _)| {
                if args.skip_all || args.skips.contains(param) {
                    return false;
                }

                // If any parameters have the same name as a custom field, skip
                // and allow them to be formatted by the custom field.
                if let Some(ref fields) = args.fields {
                    fields.0.iter().all(|Field { ref name, .. }| {
                        let first = name.first();
                        first != name.last() || !first.iter().any(|name| name == &param)
                    })
                } else {
                    true
                }
            })
            .map(|(user_name, (real_name, record_type))| match record_type {
                RecordType::Value => quote!(#user_name = #real_name),
                RecordType::Debug => quote!(#user_name = tracing::field::debug(&#real_name)),
            })
            .collect();

        // replace every use of a variable with its original name
        if let Some(Fields(ref mut fields)) = args.fields {
            let mut replacer = IdentAndTypesRenamer {
                idents: param_names.into_iter().map(|(a, (b, _))| (a, b)).collect(),
                types: Vec::new(),
            };

            // when async-trait <=0.1.43 is in use, replace instances
            // of the "Self" type inside the fields values
            if let Some(self_type) = self_type {
                replacer.types.push(("Self", self_type.clone()));
            }

            for e in fields.iter_mut().filter_map(|f| f.value.as_mut()) {
                syn::visit_mut::visit_expr_mut(&mut replacer, e);
            }
        }

        let custom_fields = &args.fields;

        quote!(tracing::span!(
            target: #target,
            #level,
            #span_name,
            #(#quoted_fields,)*
            #custom_fields

        ))
    })();

    // Generate the instrumented function body.
    // If the function is an `async fn`, this will wrap it in an async block,
    // which is `instrument`ed using `tracing-futures`. Otherwise, this will
    // enter the span and then perform the rest of the body.
    // If `err` is in args, instrument any resulting `Err`s.
    if async_context {
        let mk_fut = if err {
            quote_spanned!(block.span()=>
                async move {
                    match async move { #block }.await {
                        #[allow(clippy::unit_arg)]
                        Ok(x) => Ok(x),
                        Err(e) => {
                            tracing::error!(error = %e);
                            Err(e)
                        }
                    }
                }
            )
        } else {
            quote_spanned!(block.span()=>
                async move { #block }
            )
        };

        return quote!(
            let __tracing_attr_span = #span;
            let __tracing_instrument_future = #mk_fut;
            if !__tracing_attr_span.is_disabled() {
                tracing::Instrument::instrument(
                    __tracing_instrument_future,
                    __tracing_attr_span
                )
                .await
            } else {
                __tracing_instrument_future.await
            }
        );
    }

    let span = quote!(
        // These variables are left uninitialized and initialized only
        // if the tracing level is statically enabled at this point.
        // While the tracing level is also checked at span creation
        // time, that will still create a dummy span, and a dummy guard
        // and drop the dummy guard later. By lazily initializing these
        // variables, Rust will generate a drop flag for them and thus
        // only drop the guard if it was created. This creates code that
        // is very straightforward for LLVM to optimize out if the tracing
        // level is statically disabled, while not causing any performance
        // regression in case the level is enabled.
        let __tracing_attr_span;
        let __tracing_attr_guard;
        if tracing::level_enabled!(#level) {
            __tracing_attr_span = #span;
            __tracing_attr_guard = __tracing_attr_span.enter();
        }
    );

    if err {
        return quote_spanned!(block.span()=>
            #span
            #[allow(clippy::redundant_closure_call)]
            match (move || #block)() {
                #[allow(clippy::unit_arg)]
                Ok(x) => Ok(x),
                Err(e) => {
                    tracing::error!(error = %e);
                    Err(e)
                }
            }
        );
    }

    quote_spanned!(block.span() =>
        // Because `quote` produces a stream of tokens _without_ whitespace, the
        // `if` and the block will appear directly next to each other. This
        // generates a clippy lint about suspicious `if/else` formatting.
        // Therefore, suppress the lint inside the generated code...
        #[allow(clippy::suspicious_else_formatting)]
        {
            #span
            // ...but turn the lint back on inside the function body.
            #[warn(clippy::suspicious_else_formatting)]
            #block
        }
    )
}

#[derive(Default, Debug)]
struct InstrumentArgs {
    level: Option<Level>,
    name: Option<LitStr>,
    target: Option<LitStr>,
    skips: HashSet<Ident>,
    skip_all: bool,
    fields: Option<Fields>,
    err: bool,
    /// Errors describing any unrecognized parse inputs that we skipped.
    parse_warnings: Vec<syn::Error>,
}

impl InstrumentArgs {
    fn level(&self) -> impl ToTokens {
        fn is_level(lit: &LitInt, expected: u64) -> bool {
            match lit.base10_parse::<u64>() {
                Ok(value) => value == expected,
                Err(_) => false,
            }
        }

        match &self.level {
            Some(Level::Str(ref lit)) if lit.value().eq_ignore_ascii_case("trace") => {
                quote!(tracing::Level::TRACE)
            }
            Some(Level::Str(ref lit)) if lit.value().eq_ignore_ascii_case("debug") => {
                quote!(tracing::Level::DEBUG)
            }
            Some(Level::Str(ref lit)) if lit.value().eq_ignore_ascii_case("info") => {
                quote!(tracing::Level::INFO)
            }
            Some(Level::Str(ref lit)) if lit.value().eq_ignore_ascii_case("warn") => {
                quote!(tracing::Level::WARN)
            }
            Some(Level::Str(ref lit)) if lit.value().eq_ignore_ascii_case("error") => {
                quote!(tracing::Level::ERROR)
            }
            Some(Level::Int(ref lit)) if is_level(lit, 1) => quote!(tracing::Level::TRACE),
            Some(Level::Int(ref lit)) if is_level(lit, 2) => quote!(tracing::Level::DEBUG),
            Some(Level::Int(ref lit)) if is_level(lit, 3) => quote!(tracing::Level::INFO),
            Some(Level::Int(ref lit)) if is_level(lit, 4) => quote!(tracing::Level::WARN),
            Some(Level::Int(ref lit)) if is_level(lit, 5) => quote!(tracing::Level::ERROR),
            Some(Level::Path(ref pat)) => quote!(#pat),
            Some(lit) => quote! {
                compile_error!(
                    "unknown verbosity level, expected one of \"trace\", \
                     \"debug\", \"info\", \"warn\", or \"error\", or a number 1-5"
                )
            },
            None => quote!(tracing::Level::INFO),
        }
    }

    fn target(&self) -> impl ToTokens {
        if let Some(ref target) = self.target {
            quote!(#target)
        } else {
            quote!(module_path!())
        }
    }

    /// Generate "deprecation" warnings for any unrecognized attribute inputs
    /// that we skipped.
    ///
    /// For backwards compatibility, we need to emit compiler warnings rather
    /// than errors for unrecognized inputs. Generating a fake deprecation is
    /// the only way to do this on stable Rust right now.
    fn warnings(&self) -> impl ToTokens {
        let warnings = self.parse_warnings.iter().map(|err| {
            let msg = format!("found unrecognized input, {}", err);
            let msg = LitStr::new(&msg, err.span());
            // TODO(eliza): This is a bit of a hack, but it's just about the
            // only way to emit warnings from a proc macro on stable Rust.
            // Eventually, when the `proc_macro::Diagnostic` API stabilizes, we
            // should definitely use that instead.
            quote_spanned! {err.span()=>
                #[warn(deprecated)]
                {
                    #[deprecated(since = "not actually deprecated", note = #msg)]
                    const TRACING_INSTRUMENT_WARNING: () = ();
                    let _ = TRACING_INSTRUMENT_WARNING;
                }
            }
        });
        quote! {
            { #(#warnings)* }
        }
    }
}

impl Parse for InstrumentArgs {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut args = Self::default();
        while !input.is_empty() {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::name) {
                if args.name.is_some() {
                    return Err(input.error("expected only a single `name` argument"));
                }
                let name = input.parse::<StrArg<kw::name>>()?.value;
                args.name = Some(name);
            } else if lookahead.peek(LitStr) {
                // XXX: apparently we support names as either named args with an
                // sign, _or_ as unnamed string literals. That's weird, but
                // changing it is apparently breaking.
                if args.name.is_some() {
                    return Err(input.error("expected only a single `name` argument"));
                }
                args.name = Some(input.parse()?);
            } else if lookahead.peek(kw::target) {
                if args.target.is_some() {
                    return Err(input.error("expected only a single `target` argument"));
                }
                let target = input.parse::<StrArg<kw::target>>()?.value;
                args.target = Some(target);
            } else if lookahead.peek(kw::level) {
                if args.level.is_some() {
                    return Err(input.error("expected only a single `level` argument"));
                }
                args.level = Some(input.parse()?);
            } else if lookahead.peek(kw::skip) {
                if !args.skips.is_empty() {
                    return Err(input.error("expected only a single `skip` argument"));
                }
                if args.skip_all {
                    return Err(input.error("expected either `skip` or `skip_all` argument"));
                }
                let Skips(skips) = input.parse()?;
                args.skips = skips;
            } else if lookahead.peek(kw::skip_all) {
                if args.skip_all {
                    return Err(input.error("expected only a single `skip_all` argument"));
                }
                if !args.skips.is_empty() {
                    return Err(input.error("expected either `skip` or `skip_all` argument"));
                }
                let _ = input.parse::<kw::skip_all>()?;
                args.skip_all = true;
            } else if lookahead.peek(kw::fields) {
                if args.fields.is_some() {
                    return Err(input.error("expected only a single `fields` argument"));
                }
                args.fields = Some(input.parse()?);
            } else if lookahead.peek(kw::err) {
                let _ = input.parse::<kw::err>()?;
                args.err = true;
            } else if lookahead.peek(Token![,]) {
                let _ = input.parse::<Token![,]>()?;
            } else {
                // We found a token that we didn't expect!
                // We want to emit warnings for these, rather than errors, so
                // we'll add it to the list of unrecognized inputs we've seen so
                // far and keep going.
                args.parse_warnings.push(lookahead.error());
                // Parse the unrecognized token tree to advance the parse
                // stream, and throw it away so we can keep parsing.
                let _ = input.parse::<proc_macro2::TokenTree>();
            }
        }
        Ok(args)
    }
}

struct StrArg<T> {
    value: LitStr,
    _p: std::marker::PhantomData<T>,
}

impl<T: Parse> Parse for StrArg<T> {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let _ = input.parse::<T>()?;
        let _ = input.parse::<Token![=]>()?;
        let value = input.parse()?;
        Ok(Self {
            value,
            _p: std::marker::PhantomData,
        })
    }
}

struct Skips(HashSet<Ident>);

impl Parse for Skips {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let _ = input.parse::<kw::skip>();
        let content;
        let _ = syn::parenthesized!(content in input);
        let names: Punctuated<Ident, Token![,]> = content.parse_terminated(Ident::parse_any)?;
        let mut skips = HashSet::new();
        for name in names {
            if skips.contains(&name) {
                return Err(syn::Error::new(
                    name.span(),
                    "tried to skip the same field twice",
                ));
            } else {
                skips.insert(name);
            }
        }
        Ok(Self(skips))
    }
}

#[derive(Debug)]
struct Fields(Punctuated<Field, Token![,]>);

#[derive(Debug)]
struct Field {
    name: Punctuated<Ident, Token![.]>,
    value: Option<Expr>,
    kind: FieldKind,
}

#[derive(Debug, Eq, PartialEq)]
enum FieldKind {
    Debug,
    Display,
    Value,
}

impl Parse for Fields {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let _ = input.parse::<kw::fields>();
        let content;
        let _ = syn::parenthesized!(content in input);
        let fields: Punctuated<_, Token![,]> = content.parse_terminated(Field::parse)?;
        Ok(Self(fields))
    }
}

impl ToTokens for Fields {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens)
    }
}

impl Parse for Field {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut kind = FieldKind::Value;
        if input.peek(Token![%]) {
            input.parse::<Token![%]>()?;
            kind = FieldKind::Display;
        } else if input.peek(Token![?]) {
            input.parse::<Token![?]>()?;
            kind = FieldKind::Debug;
        };
        let name = Punctuated::parse_separated_nonempty_with(input, Ident::parse_any)?;
        let value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            if input.peek(Token![%]) {
                input.parse::<Token![%]>()?;
                kind = FieldKind::Display;
            } else if input.peek(Token![?]) {
                input.parse::<Token![?]>()?;
                kind = FieldKind::Debug;
            };
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Self { name, value, kind })
    }
}

impl ToTokens for Field {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(ref value) = self.value {
            let name = &self.name;
            let kind = &self.kind;
            tokens.extend(quote! {
                #name = #kind#value
            })
        } else if self.kind == FieldKind::Value {
            // XXX(eliza): I don't like that fields without values produce
            // empty fields rather than local variable shorthand...but,
            // we've released a version where field names without values in
            // `instrument` produce empty field values, so changing it now
            // is a breaking change. agh.
            let name = &self.name;
            tokens.extend(quote!(#name = tracing::field::Empty))
        } else {
            self.kind.to_tokens(tokens);
            self.name.to_tokens(tokens);
        }
    }
}

impl ToTokens for FieldKind {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            FieldKind::Debug => tokens.extend(quote! { ? }),
            FieldKind::Display => tokens.extend(quote! { % }),
            _ => {}
        }
    }
}

#[derive(Debug)]
enum Level {
    Str(LitStr),
    Int(LitInt),
    Path(Path),
}

impl Parse for Level {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let _ = input.parse::<kw::level>()?;
        let _ = input.parse::<Token![=]>()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(LitStr) {
            Ok(Self::Str(input.parse()?))
        } else if lookahead.peek(LitInt) {
            Ok(Self::Int(input.parse()?))
        } else if lookahead.peek(Ident) {
            Ok(Self::Path(input.parse()?))
        } else {
            Err(lookahead.error())
        }
    }
}

/// Indicates whether a field should be recorded as `Value` or `Debug`.
enum RecordType {
    /// The field should be recorded using its `Value` implementation.
    Value,
    /// The field should be recorded using `tracing::field::debug()`.
    Debug,
}

impl RecordType {
    /// Array of primitive types which should be recorded as [RecordType::Value].
    const TYPES_FOR_VALUE: &'static [&'static str] = &[
        "bool",
        "str",
        "u8",
        "i8",
        "u16",
        "i16",
        "u32",
        "i32",
        "u64",
        "i64",
        "f32",
        "f64",
        "usize",
        "isize",
        "NonZeroU8",
        "NonZeroI8",
        "NonZeroU16",
        "NonZeroI16",
        "NonZeroU32",
        "NonZeroI32",
        "NonZeroU64",
        "NonZeroI64",
        "NonZeroUsize",
        "NonZeroIsize",
        "Wrapping",
    ];

    /// Parse `RecordType` from [syn::Type] by looking up
    /// the [RecordType::TYPES_FOR_VALUE] array.
    fn parse_from_ty(ty: &syn::Type) -> Self {
        match ty {
            syn::Type::Path(syn::TypePath { path, .. })
                if path
                    .segments
                    .iter()
                    .last()
                    .map(|path_segment| {
                        let ident = path_segment.ident.to_string();
                        Self::TYPES_FOR_VALUE.iter().any(|&t| t == ident)
                    })
                    .unwrap_or(false) =>
            {
                RecordType::Value
            }
            syn::Type::Reference(syn::TypeReference { elem, .. }) => {
                RecordType::parse_from_ty(&*elem)
            }
            _ => RecordType::Debug,
        }
    }
}

fn param_names(pat: Pat, record_type: RecordType) -> Box<dyn Iterator<Item = (Ident, RecordType)>> {
    match pat {
        Pat::Ident(PatIdent { ident, .. }) => Box::new(iter::once((ident, record_type))),
        Pat::Reference(PatReference { pat, .. }) => param_names(*pat, record_type),
        // We can't get the concrete type of fields in the struct/tuple
        // patterns by using `syn`. e.g. `fn foo(Foo { x, y }: Foo) {}`.
        // Therefore, the struct/tuple patterns in the arguments will just
        // always be recorded as `RecordType::Debug`.
        Pat::Struct(PatStruct { fields, .. }) => Box::new(
            fields
                .into_iter()
                .flat_map(|FieldPat { pat, .. }| param_names(*pat, RecordType::Debug)),
        ),
        Pat::Tuple(PatTuple { elems, .. }) => Box::new(
            elems
                .into_iter()
                .flat_map(|p| param_names(p, RecordType::Debug)),
        ),
        Pat::TupleStruct(PatTupleStruct {
            pat: PatTuple { elems, .. },
            ..
        }) => Box::new(
            elems
                .into_iter()
                .flat_map(|p| param_names(p, RecordType::Debug)),
        ),

        // The above *should* cover all cases of irrefutable patterns,
        // but we purposefully don't do any funny business here
        // (such as panicking) because that would obscure rustc's
        // much more informative error message.
        _ => Box::new(iter::empty()),
    }
}

mod kw {
    syn::custom_keyword!(fields);
    syn::custom_keyword!(skip);
    syn::custom_keyword!(skip_all);
    syn::custom_keyword!(level);
    syn::custom_keyword!(target);
    syn::custom_keyword!(name);
    syn::custom_keyword!(err);
}

enum AsyncTraitKind<'a> {
    // old construction. Contains the function
    Function(&'a ItemFn),
    // new construction. Contains a reference to the async block
    Async(&'a ExprAsync),
}

struct AsyncTraitInfo<'a> {
    // statement that must be patched
    source_stmt: &'a Stmt,
    kind: AsyncTraitKind<'a>,
    self_type: Option<syn::TypePath>,
}

// Get the AST of the inner function we need to hook, if it was generated
// by async-trait.
// When we are given a function annotated by async-trait, that function
// is only a placeholder that returns a pinned future containing the
// user logic, and it is that pinned future that needs to be instrumented.
// Were we to instrument its parent, we would only collect information
// regarding the allocation of that future, and not its own span of execution.
// Depending on the version of async-trait, we inspect the block of the function
// to find if it matches the pattern
// `async fn foo<...>(...) {...}; Box::pin(foo<...>(...))` (<=0.1.43), or if
// it matches `Box::pin(async move { ... }) (>=0.1.44). We the return the
// statement that must be instrumented, along with some other informations.
// 'gen_body' will then be able to use that information to instrument the
// proper function/future.
// (this follows the approach suggested in
// https://github.com/dtolnay/async-trait/issues/45#issuecomment-571245673)
fn get_async_trait_info(block: &Block, block_is_async: bool) -> Option<AsyncTraitInfo<'_>> {
    // are we in an async context? If yes, this isn't a async_trait-like pattern
    if block_is_async {
        return None;
    }

    // list of async functions declared inside the block
    let inside_funs = block.stmts.iter().filter_map(|stmt| {
        if let Stmt::Item(Item::Fn(fun)) = &stmt {
            // If the function is async, this is a candidate
            if fun.sig.asyncness.is_some() {
                return Some((stmt, fun));
            }
        }
        None
    });

    // last expression of the block (it determines the return value
    // of the block, so that if we are working on a function whose
    // `trait` or `impl` declaration is annotated by async_trait,
    // this is quite likely the point where the future is pinned)
    let (last_expr_stmt, last_expr) = block.stmts.iter().rev().find_map(|stmt| {
        if let Stmt::Expr(expr) = stmt {
            Some((stmt, expr))
        } else {
            None
        }
    })?;

    // is the last expression a function call?
    let (outside_func, outside_args) = match last_expr {
        Expr::Call(ExprCall { func, args, .. }) => (func, args),
        _ => return None,
    };

    // is it a call to `Box::pin()`?
    let path = match outside_func.as_ref() {
        Expr::Path(path) => &path.path,
        _ => return None,
    };
    if !path_to_string(path).ends_with("Box::pin") {
        return None;
    }

    // Does the call take an argument? If it doesn't,
    // it's not gonna compile anyway, but that's no reason
    // to (try to) perform an out of bounds access
    if outside_args.is_empty() {
        return None;
    }

    // Is the argument to Box::pin an async block that
    // captures its arguments?
    if let Expr::Async(async_expr) = &outside_args[0] {
        // check that the move 'keyword' is present
        async_expr.capture?;

        return Some(AsyncTraitInfo {
            source_stmt: last_expr_stmt,
            kind: AsyncTraitKind::Async(async_expr),
            self_type: None,
        });
    }

    // Is the argument to Box::pin a function call itself?
    let func = match &outside_args[0] {
        Expr::Call(ExprCall { func, .. }) => func,
        _ => return None,
    };

    // "stringify" the path of the function called
    let func_name = match **func {
        Expr::Path(ref func_path) => path_to_string(&func_path.path),
        _ => return None,
    };

    // Was that function defined inside of the current block?
    // If so, retrieve the statement where it was declared and the function itself
    let (stmt_func_declaration, func) = inside_funs
        .into_iter()
        .find(|(_, fun)| fun.sig.ident == func_name)?;

    // If "_self" is present as an argument, we store its type to be able to rewrite "Self" (the
    // parameter type) with the type of "_self"
    let mut self_type = None;
    for arg in &func.sig.inputs {
        if let FnArg::Typed(ty) = arg {
            if let Pat::Ident(PatIdent { ref ident, .. }) = *ty.pat {
                if ident == "_self" {
                    let mut ty = *ty.ty.clone();
                    // extract the inner type if the argument is "&self" or "&mut self"
                    if let syn::Type::Reference(syn::TypeReference { elem, .. }) = ty {
                        ty = *elem;
                    }

                    if let syn::Type::Path(tp) = ty {
                        self_type = Some(tp);
                        break;
                    }
                }
            }
        }
    }

    Some(AsyncTraitInfo {
        source_stmt: stmt_func_declaration,
        kind: AsyncTraitKind::Function(func),
        self_type,
    })
}

// Return a path as a String
fn path_to_string(path: &Path) -> String {
    use std::fmt::Write;
    // some heuristic to prevent too many allocations
    let mut res = String::with_capacity(path.segments.len() * 5);
    for i in 0..path.segments.len() {
        write!(&mut res, "{}", path.segments[i].ident)
            .expect("writing to a String should never fail");
        if i < path.segments.len() - 1 {
            res.push_str("::");
        }
    }
    res
}

/// A visitor struct to replace idents and types in some piece
/// of code (e.g. the "self" and "Self" tokens in user-supplied
/// fields expressions when the function is generated by an old
/// version of async-trait).
struct IdentAndTypesRenamer<'a> {
    types: Vec<(&'a str, TypePath)>,
    idents: Vec<(Ident, Ident)>,
}

impl<'a> syn::visit_mut::VisitMut for IdentAndTypesRenamer<'a> {
    // we deliberately compare strings because we want to ignore the spans
    // If we apply clippy's lint, the behavior changes
    #[allow(clippy::cmp_owned)]
    fn visit_ident_mut(&mut self, id: &mut Ident) {
        for (old_ident, new_ident) in &self.idents {
            if id.to_string() == old_ident.to_string() {
                *id = new_ident.clone();
            }
        }
    }

    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        for (type_name, new_type) in &self.types {
            if let syn::Type::Path(TypePath { path, .. }) = ty {
                if path_to_string(path) == *type_name {
                    *ty = syn::Type::Path(new_type.clone());
                }
            }
        }
    }
}

// A visitor struct that replace an async block by its patched version
struct AsyncTraitBlockReplacer<'a> {
    block: &'a Block,
    patched_block: Block,
}

impl<'a> syn::visit_mut::VisitMut for AsyncTraitBlockReplacer<'a> {
    fn visit_block_mut(&mut self, i: &mut Block) {
        if i == self.block {
            *i = self.patched_block.clone();
        }
    }
}

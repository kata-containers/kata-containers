Parser for Rust source code
===========================

[![Build Status](https://api.travis-ci.org/dtolnay/syn.svg?branch=master)](https://travis-ci.org/dtolnay/syn)
[![Latest Version](https://img.shields.io/crates/v/syn.svg)](https://crates.io/crates/syn)
[![Rust Documentation](https://img.shields.io/badge/api-rustdoc-blue.svg)](https://docs.rs/syn/0.15/syn/)
[![Rustc Version 1.15+](https://img.shields.io/badge/rustc-1.15+-lightgray.svg)](https://blog.rust-lang.org/2017/02/02/Rust-1.15.html)

Syn is a parsing library for parsing a stream of Rust tokens into a syntax tree
of Rust source code.

Currently this library is geared toward use in Rust procedural macros, but
contains some APIs that may be useful more generally.

[custom derive]: https://github.com/rust-lang/rfcs/blob/master/text/1681-macros-1.1.md

- **Data structures** — Syn provides a complete syntax tree that can represent
  any valid Rust source code. The syntax tree is rooted at [`syn::File`] which
  represents a full source file, but there are other entry points that may be
  useful to procedural macros including [`syn::Item`], [`syn::Expr`] and
  [`syn::Type`].

- **Custom derives** — Of particular interest to custom derives is
  [`syn::DeriveInput`] which is any of the three legal input items to a derive
  macro. An example below shows using this type in a library that can derive
  implementations of a trait of your own.

- **Parsing** — Parsing in Syn is built around [parser functions] with the
  signature `fn(ParseStream) -> Result<T>`. Every syntax tree node defined by
  Syn is individually parsable and may be used as a building block for custom
  syntaxes, or you may dream up your own brand new syntax without involving any
  of our syntax tree types.

- **Location information** — Every token parsed by Syn is associated with a
  `Span` that tracks line and column information back to the source of that
  token. These spans allow a procedural macro to display detailed error messages
  pointing to all the right places in the user's code. There is an example of
  this below.

- **Feature flags** — Functionality is aggressively feature gated so your
  procedural macros enable only what they need, and do not pay in compile time
  for all the rest.

[`syn::File`]: https://docs.rs/syn/0.15/syn/struct.File.html
[`syn::Item`]: https://docs.rs/syn/0.15/syn/enum.Item.html
[`syn::Expr`]: https://docs.rs/syn/0.15/syn/enum.Expr.html
[`syn::Type`]: https://docs.rs/syn/0.15/syn/enum.Type.html
[`syn::DeriveInput`]: https://docs.rs/syn/0.15/syn/struct.DeriveInput.html
[parser functions]: https://docs.rs/syn/0.15/syn/parse/index.html

If you get stuck with anything involving procedural macros in Rust I am happy to
provide help even if the issue is not related to Syn. Please file a ticket in
this repo.

*Version requirement: Syn supports any compiler version back to Rust's very
first support for procedural macros in Rust 1.15.0. Some features especially
around error reporting are only available in newer compilers or on the nightly
channel.*

[*Release notes*](https://github.com/dtolnay/syn/releases)

## Example of a custom derive

The canonical custom derive using Syn looks like this. We write an ordinary Rust
function tagged with a `proc_macro_derive` attribute and the name of the trait
we are deriving. Any time that derive appears in the user's code, the Rust
compiler passes their data structure as tokens into our macro. We get to execute
arbitrary Rust code to figure out what to do with those tokens, then hand some
tokens back to the compiler to compile into the user's crate.

[`TokenStream`]: https://doc.rust-lang.org/proc_macro/struct.TokenStream.html

```toml
[dependencies]
syn = "0.15"
quote = "0.6"

[lib]
proc-macro = true
```

```rust
extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(MyMacro)]
pub fn my_macro(input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let input = parse_macro_input!(input as DeriveInput);

    // Build the output, possibly using quasi-quotation
    let expanded = quote! {
        // ...
    };

    // Hand the output tokens back to the compiler
    TokenStream::from(expanded)
}
```

The [`heapsize`] example directory shows a complete working Macros 1.1
implementation of a custom derive. It works on any Rust compiler 1.15+. The
example derives a `HeapSize` trait which computes an estimate of the amount of
heap memory owned by a value.

[`heapsize`]: examples/heapsize

```rust
pub trait HeapSize {
    /// Total number of bytes of heap memory owned by `self`.
    fn heap_size_of_children(&self) -> usize;
}
```

The custom derive allows users to write `#[derive(HeapSize)]` on data structures
in their program.

```rust
#[derive(HeapSize)]
struct Demo<'a, T: ?Sized> {
    a: Box<T>,
    b: u8,
    c: &'a str,
    d: String,
}
```

## Spans and error reporting

The token-based procedural macro API provides great control over where the
compiler's error messages are displayed in user code. Consider the error the
user sees if one of their field types does not implement `HeapSize`.

```rust
#[derive(HeapSize)]
struct Broken {
    ok: String,
    bad: std::thread::Thread,
}
```

By tracking span information all the way through the expansion of a procedural
macro as shown in the `heapsize` example, token-based macros in Syn are able to
trigger errors that directly pinpoint the source of the problem.

```
error[E0277]: the trait bound `std::thread::Thread: HeapSize` is not satisfied
 --> src/main.rs:7:5
  |
7 |     bad: std::thread::Thread,
  |     ^^^^^^^^^^^^^^^^^^^^^^^^ the trait `HeapSize` is not implemented for `std::thread::Thread`
```

## Parsing a custom syntax

The [`lazy-static`] example directory shows the implementation of a
`functionlike!(...)` procedural macro in which the input tokens are parsed using
Syn's parsing API.

[`lazy-static`]: examples/lazy-static

The example reimplements the popular `lazy_static` crate from crates.io as a
procedural macro.

```
lazy_static! {
    static ref USERNAME: Regex = Regex::new("^[a-z0-9_-]{3,16}$").unwrap();
}
```

The implementation shows how to trigger custom warnings and error messages on
the macro input.

```
warning: come on, pick a more creative name
  --> src/main.rs:10:16
   |
10 |     static ref FOO: String = "lazy_static".to_owned();
   |                ^^^
```

## Testing

When testing macros, we often care not just that the macro can be used
successfully but also that when the macro is provided with invalid input it
produces maximally helpful error messages. Consider using the [`trybuild`] crate
to write tests for errors that are emitted by your macro or errors detected by
the Rust compiler in the expanded code following misuse of the macro. Such tests
help avoid regressions from later refactors that mistakenly make an error no
longer trigger or be less helpful than it used to be.

[`trybuild`]: https://github.com/dtolnay/trybuild

## Debugging

When developing a procedural macro it can be helpful to look at what the
generated code looks like. Use `cargo rustc -- -Zunstable-options
--pretty=expanded` or the [`cargo expand`] subcommand.

[`cargo expand`]: https://github.com/dtolnay/cargo-expand

To show the expanded code for some crate that uses your procedural macro, run
`cargo expand` from that crate. To show the expanded code for one of your own
test cases, run `cargo expand --test the_test_case` where the last argument is
the name of the test file without the `.rs` extension.

This write-up by Brandon W Maister discusses debugging in more detail:
[Debugging Rust's new Custom Derive system][debugging].

[debugging]: https://quodlibetor.github.io/posts/debugging-rusts-new-custom-derive-system/

## Optional features

Syn puts a lot of functionality behind optional features in order to optimize
compile time for the most common use cases. The following features are
available.

- **`derive`** *(enabled by default)* — Data structures for representing the
  possible input to a custom derive, including structs and enums and types.
- **`full`** — Data structures for representing the syntax tree of all valid
  Rust source code, including items and expressions.
- **`parsing`** *(enabled by default)* — Ability to parse input tokens into a
  syntax tree node of a chosen type.
- **`printing`** *(enabled by default)* — Ability to print a syntax tree node as
  tokens of Rust source code.
- **`visit`** — Trait for traversing a syntax tree.
- **`visit-mut`** — Trait for traversing and mutating in place a syntax tree.
- **`fold`** — Trait for transforming an owned syntax tree.
- **`clone-impls`** *(enabled by default)* — Clone impls for all syntax tree
  types.
- **`extra-traits`** — Debug, Eq, PartialEq, Hash impls for all syntax tree
  types.
- **`proc-macro`** *(enabled by default)* — Runtime dependency on the dynamic
  library libproc_macro from rustc toolchain.

## Proc macro shim

Syn uses the [proc-macro2] crate to emulate the compiler's procedural macro API
in a stable way that works all the way back to Rust 1.15.0. This shim makes it
possible to write code without regard for whether the current compiler version
supports the features we use.

In general all of your code should be written against proc-macro2 rather than
proc-macro. The one exception is in the signatures of procedural macro entry
points, which are required by the language to use `proc_macro::TokenStream`.

The proc-macro2 crate will automatically detect and use the compiler's data
structures on sufficiently new compilers.

[proc-macro2]: https://github.com/alexcrichton/proc-macro2

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>

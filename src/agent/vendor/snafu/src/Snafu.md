The `Snafu` macro is the entrypoint to defining your own error
types. It is designed to require little configuration for the
recommended and typical usecases while still offering flexibility for
unique situations.

- [`backtrace`](#controlling-backtraces)
- [`context`](#controlling-context)
- [`crate_root`](#controlling-how-the-snafu-crate-is-resolved)
- [`display`](#controlling-display)
- [`implicit`](#controlling-implicitly-generated-data)
- [`module`](#placing-context-selectors-in-modules)
- [`provide`](#providing-data-beyond-the-error-trait)
- [`source`](#controlling-error-sources)
- [`visibility`](#controlling-visibility)
- [`whatever`](#controlling-stringly-typed-errors)

## Attribute cheat sheet

Use this as a quick reminder of what each attribute can do and where
it is valid. Detailed information on each attribute is below.

### Enum

| Option (inside `#[snafu(...)]`) | Description                                                                                                 |
|---------------------------------|-------------------------------------------------------------------------------------------------------------|
| `visibility(V)`                 | Sets the default visibility of the generated context selectors to `V` (e.g. `pub`)                          |
| `module`                        | Puts the generated context selectors into a module (module name is the enum name converted to `snake_case`) |
| `module(N)`                     | Same as above, but with the module named `N` instead                                                        |
| `context(suffix(N))`            | Changes the default context selector suffix from `Snafu` to `N`                                             |
| `crate_root(C)`                 | Generated code refers to a crate named `C` instead of the default `snafu`                                   |

### Enum variant or struct

| Option (inside `#[snafu(...)]`) | Description                                                                                                                                                                                     |
|---------------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `display("{field:?}: {}", foo)` | Sets the display implementation for this error variant using `format_args!` syntax. If this is omitted, the default is `"VariantName: {source}"` if there is a source or `"VariantName"` if not |
| `context(false)`                | Skips creation of the context selector, implements `From` for the mandatory source error                                                                                                        |
| `context(suffix(N))`            | Changes the suffix of the generated context selector to `N`                                                                                                                                     |
| `context(suffix(false))`        | No suffix for the generated context selector                                                                                                                                                    |
| `visibility(v)`                 | Sets the visibility of the generated context selector to `v` (e.g. `pub`)                                                                                                                       |
| `visibility`                    | Resets visibility back to private                                                                                                                                                               |
| `provide(flags, type => expr)`  | Provides the type using the `expr` with the optional flags                                                                                                                                      |
| `whatever`                      | Stringly-typed error. Message field must be called `message`. Source optional, but if present must be of a specific [format](#controlling-stringly-typed-errors)                                |

### Context fields

| Option (inside `#[snafu(...)]`) | Description                                                                                               |
|---------------------------------|-----------------------------------------------------------------------------------------------------------|
| `source`                        | Marks a field as the source error (even if not called `source`)                                           |
| `source(from(type, transform))` | As above, plus converting from `type` to the field type by calling `transform`                            |
| `source(false)`                 | Marks a field that is named `source` as a regular field                                                   |
| `backtrace`                     | Marks a field as backtrace (even if not called `backtrace`)                                               |
| `backtrace(false)`              | Marks a field that is named `backtrace` as a regular field                                                |
| `implicit`                      | Marks a field as implicit (Type needs to implement [`GenerateImplicitData`](crate::GenerateImplicitData)) |
| `provide`                       | Marks a field as providing a reference to the type                                                        |

## Controlling `Display`

You can specify how the `Display` trait will be implemented for each
variant. The argument is a format string and the arguments. All of the
fields of the variant will be available and you can call methods on
them, such as `filename.display()`. As an extension to the current
format string capabilities, a shorthand is available for named
arguments that match a field.

**Example**

```rust
# use snafu::prelude::*;
#[derive(Debug, Snafu)]
enum Error {
    #[snafu(display("{username} may not log in until they pay USD {amount:E}"))]
    UserMustPayForService { username: String, amount: f32 },
}
fn main() {
    assert_eq!(
        UserMustPayForServiceSnafu {
            username: "Stefani",
            amount: 1_000_000.0,
        }
        .build()
        .to_string(),
        "Stefani may not log in until they pay USD 1E6",
    );
}
```

### The default `Display` implementation

It is recommended that you provide a value for `snafu(display)`, but
if it is omitted, the summary of the documentation comment will be
used. If that is not present, the name of the variant will be used.

```rust
# use snafu::prelude::*;
#[derive(Debug, Snafu)]
enum Error {
    /// No user available.
    /// You may need to specify one.
    MissingUser,
    MissingPassword,
}

fn main() {
    assert_eq!(
        MissingUserSnafu.build().to_string(),
        "No user available. You may need to specify one.",
    );
    assert_eq!(MissingPasswordSnafu.build().to_string(), "MissingPassword");
}
```

## Controlling context

### Changing the context selector suffix

When context selectors are generated, any `Error` suffix is removed
and the suffix `Snafu` is added by default. If you'd prefer a
different suffix, such as `Ctx` or `Context`, you can specify that
with `#[snafu(context(suffix(SomeIdentifier)))]`. If you'd like to
disable the suffix entirely, you can use
`#[snafu(context(suffix(false)))]`.

**Example**

```rust
# use snafu::prelude::*;
#
#[derive(Debug, Snafu)]
enum Error {
    UsesTheDefaultSuffixError,

    #[snafu(context(suffix(Ctx)))]
    HasAnotherSuffix,

    #[snafu(context(suffix(false)))]
    DoesNotHaveASuffix,
}

fn my_code() -> Result<(), Error> {
    UsesTheDefaultSuffixSnafu.fail()?;

    HasAnotherSuffixCtx.fail()?;

    DoesNotHaveASuffix.fail()?;

    Ok(())
}
```

`#[snafu(context(suffix))]` can be specified on an enum as the default
suffix for variants of the enum. In that case, if you wish to have one
variant with a suffix, you will need to express it explicitly with
`#[snafu(context(suffix(SomeIdentifier)))]`.

### Disabling the context selector

Sometimes, an underlying error can only occur in exactly one context
and there's no additional information that can be provided to the
caller. In these cases, you can use `#[snafu(context(false))]` to
indicate that no context selector should be created. This allows using
the `?` operator directly on the underlying error.

Please think about your end users before making liberal use of this
feature. Adding context to an error is often what distinguishes an
actionable error from a frustrating one.

**Example**

```rust
# use snafu::prelude::*;
#
#[derive(Debug, Snafu)]
enum Error {
    #[snafu(context(false))]
    NeedsNoIntroduction { source: VeryUniqueError },
}

fn my_code() -> Result<i32, Error> {
    let val = do_something_unique()?;
    Ok(val + 10)
}

# #[derive(Debug, Snafu)]
# enum VeryUniqueError {}
fn do_something_unique() -> Result<i32, VeryUniqueError> {
    // ...
#    Ok(42)
}
```

## Controlling visibility

By default, each of the context selectors and their inherent
methods will be private. It is our opinion that each module should
have one or more error types that are scoped to that module,
reducing the need to deal with unrelated errors when matching and
increasing cohesiveness.

If you need to access the context selectors from outside of their
module, you can use the `#[snafu(visibility)]` attribute. This can
be applied to the error type as a default visibility or to
specific context selectors.

There are multiple forms of the attribute:

- `#[snafu(visibility(X))]`

  `X` is a normal Rust visibility modifier (`pub`, `pub(crate)`,
  `pub(in some::path)`, etc.).

- `#[snafu(visibility)]` will reset back to private visibility.

```
# use snafu::prelude::*;
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))] // Sets the default visibility for these context selectors
pub(crate) enum Error {
    IsPubCrate, // Uses the default
    #[snafu(visibility)]
    IsPrivate, // Will be private
}
```

It should be noted that API stability of context selectors is not
guaranteed. Therefore, exporting them in a crate's public API
could cause semver breakage for such crates, should SNAFU internals
change.

## Placing context selectors in modules

When you have multiple error enums that would generate conflicting
context selectors, you can choose to place the context selectors into
a module using `snafu(module)`:

```rust
use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(module)]
enum ReadError {
    Opening,
}

fn example() -> Result<(), ReadError> {
    read_error::OpeningSnafu.fail()
}

#[derive(Debug, Snafu)]
enum WriteError {
    Opening, // Would conflict if `snafu(module)` was not used above.
}
# // https://github.com/rust-lang/rust/issues/83583
# fn main() {}
```

By default, the module name will be the `snake_case` equivalent of the
enum name. You can override the default by providing an argument to
`#[snafu(module(...))]`:

```rust
use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(module(read))]
enum ReadError {
    Opening,
}

fn example() -> Result<(), ReadError> {
    read::OpeningSnafu.fail()
}
# // https://github.com/rust-lang/rust/issues/83583
# fn main() {}
```

As placing the context selectors in a module naturally namespaces
them, you may wish to combine this option with
`#[snafu(context(suffix(false)))]`:

```rust
use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(module, context(suffix(false)))]
enum ReadError {
    Opening,
}

fn example() -> Result<(), ReadError> {
    read_error::Opening.fail()
}
# // https://github.com/rust-lang/rust/issues/83583
# fn main() {}
```

The generated module starts with `use super::*`, so any types or
traits used by the context selectors need to be in scope — complicated
paths may need to be simplified or made absolute.

By default, the visibility of the generated module will be private
while the context selectors inside will be `pub(super)`. Using
[`#[snafu(visibility)]`](#controlling-visibility) to control the
visibility will change the visibility of *both* the module and the
context selectors.

## Controlling error sources

### Selecting the source field

If your error enum variant contains other errors but the field
cannot be named `source`, or if it contains a field named `source`
which is not actually an error, you can use `#[snafu(source)]` to
indicate if a field is an underlying cause or not:

```rust
# mod another {
#     use snafu::prelude::*;
#     #[derive(Debug, Snafu)]
#     pub enum Error {}
# }
# use snafu::prelude::*;
#[derive(Debug, Snafu)]
enum Error {
    SourceIsNotAnError {
        #[snafu(source(false))]
        source: String,
    },

    CauseIsAnError {
        #[snafu(source)]
        cause: another::Error,
    },
}
```

### Transforming the source

If your error type contains an underlying cause that needs to be
transformed, you can use `#[snafu(source(from(...)))]`. This takes
two arguments: the real type and an expression to transform from
that type to the type held by the error.

```rust
# mod another {
#     use snafu::prelude::*;
#     #[derive(Debug, Snafu)]
#     pub enum Error {}
# }
# use snafu::prelude::*;
#[derive(Debug, Snafu)]
enum Error {
    SourceNeedsToBeBoxed {
        #[snafu(source(from(another::Error, Box::new)))]
        source: Box<another::Error>,
    },
}

#[derive(Debug, Snafu)]
#[snafu(source(from(Error, Box::new)))]
struct ApiError(Box<Error>);
```

Note: If you specify `#[snafu(source(from(...)))]` then the field
will be treated as a source, even if it's not named "source" - in
other words, `#[snafu(source(from(...)))]` implies
`#[snafu(source)]`.

## Controlling backtraces

If your error enum variant contains a backtrace but the field
cannot be named `backtrace`, or if it contains a field named
`backtrace` which is not actually a backtrace, you can use
`#[snafu(backtrace)]` to indicate if a field is actually a
 backtrace or not:

```rust
# use snafu::{prelude::*, Backtrace};
#[derive(Debug, Snafu)]
enum Error {
    BacktraceIsNotABacktrace {
        #[snafu(backtrace(false))]
        backtrace: bool,
    },

    TraceIsABacktrace {
        #[snafu(backtrace)]
        trace: Backtrace,
    },
}
```

If your error contains other SNAFU errors which can report
backtraces, you may wish to delegate returning a backtrace to
those errors. To specify this, use `#[snafu(backtrace)]` on the
source field representing the other error:

```rust
# mod another {
#     use snafu::prelude::*;
#     #[derive(Debug, Snafu)]
#     pub enum Error {}
# }
# use snafu::prelude::*;
#[derive(Debug, Snafu)]
enum Error {
    MyError {
        #[snafu(backtrace)]
        source: another::Error,
    },
}
```

## Providing data beyond the `Error` trait

When the [`unstable-provider-api` feature flag][] is enabled, errors
will implement the standard library's [`Provider` API][provider
API]. This allows arbitrary data to be associated with an error
instance, expanding the abilities of the receiver of the error:

```rust,ignore
use snafu::prelude::*;

#[derive(Debug)]
struct UserId(u8);

#[derive(Debug, Snafu)]
enum ApiError {
    Login {
        #[snafu(provide)]
        user_id: UserId,
    },

    Logout {
        #[snafu(provide)]
        user_id: UserId,
    },

    NetworkUnreachable {
        source: std::io::Error,
    },
}

let e = LoginSnafu { user_id: UserId(0) }.build();
let e = &e as &dyn std::error::Error;
match e.request_ref::<UserId>() {
    // Present when ApiError::Login or ApiError::Logout
    Some(user_id) => {
        println!("{user_id:?} experienced an error");
    }
    // Absent when ApiError::NetworkUnreachable
    None => {
        println!("An error occurred for an unknown user");
    }
}
```

This attribute may be used even when the [`unstable-provider-api`
feature flag][] is not enabled. In that case, the attribute will be
parsed but no code will be generated, allowing library authors to
provide data to consumers willing to use nightly without losing
support for stable Rust.

[`unstable-provider-api` feature flag]: guide::feature_flags#unstable-provider-api

### Automatically provided data

By default, `source` and `backtrace` fields are exposed to the
provider API. Additionally, any data provided by the wrapped error
will be available on the wrapping error:

```rust,ignore
use snafu::{prelude::*, IntoError};

#[derive(Debug)]
struct UserId(u8);

#[derive(Debug, Snafu)]
struct InnerError {
    #[snafu(provide)]
    user_id: UserId,
    backtrace: snafu::Backtrace,
}

#[derive(Debug, Snafu)]
struct OuterError {
    source: InnerError,
}

let outer = OuterSnafu.into_error(InnerSnafu { user_id: UserId(0) }.build());
let outer = &outer as &dyn std::error::Error;

// We can get the source error and downcast it at once
outer
    .request_ref::<InnerError>()
    .expect("Must have a source");

// We can get the deepest backtrace
outer
    .request_ref::<snafu::Backtrace>()
    .expect("Must have a backtrace");

// We can get arbitrary values from sources as well
outer.request_ref::<UserId>().expect("Must have a user id");
```

By default, SNAFU will gather the provided data from the source first,
before providing any data from the current error. This can be
overridden through the [`priority` flag][provide-flag-priority].

### Manually provided data

When used on a field, the `#[snafu(provide)]` attribute will expose
that field as a reference, allowing it to be used with
[`request_ref`][]. For more control, the `#[snafu(provide)]` attribute
can be placed on the error struct or enum variant. In this location,
you supply a type and an expression that will generate that type:

```rust,ignore
use snafu::prelude::*;

#[derive(Debug, PartialEq)]
struct HttpCode(u16);

const HTTP_NOT_FOUND: HttpCode = HttpCode(404);

#[derive(Debug, Snafu)]
#[snafu(provide(HttpCode => HTTP_NOT_FOUND))]
struct WebserverError;

let e = WebserverError;
let e = &e as &dyn std::error::Error;
assert_eq!(Some(HTTP_NOT_FOUND), e.request_value::<HttpCode>());
```

The expression may access any field of the error as well as `self`:

```rust,ignore
use snafu::prelude::*;

#[derive(Debug, PartialEq)]
struct Summation(u8);

#[derive(Debug, Snafu)]
#[snafu(provide(Summation => Summation(left_side + right_side)))]
struct AdditionError {
    left_side: u8,
    right_side: u8,
}

let e = AdditionSnafu {
    left_side: 1,
    right_side: 2,
}
.build();
let e = &e as &dyn std::error::Error;
assert_eq!(Some(Summation(3)), e.request_value::<Summation>());
```

### Configuring how data is provided

You may also provide a number of optional flags that control how the
provided data will be exposed. These flags may be combined as required
and may be provided in any order.

#### `provide(ref, ...`

[provide-flag-ref]: #provideref-

Provides the data as a reference instead of as a value. The reference
must live as long as the error itself.

```rust,ignore
use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(provide(ref, str => name))]
struct RefFlagExampleError {
    name: String,
}

let e = RefFlagExampleSnafu { name: "alice" }.build();
let e = &e as &dyn std::error::Error;

assert_eq!(Some("alice"), e.request_ref::<str>());
```

#### `provide(opt, ...`

[provide-flag-opt]: #provideopt-

If the data being provided is an `Option<T>`, the `opt` flag will
flatten the data, allowing you to request `T` instead of `Option<T>`.

```rust,ignore
use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(provide(opt, char => char::from_u32(*char_code)))]
struct OptFlagExampleError {
    char_code: u32,
}

let e = OptFlagExampleSnafu { char_code: b'x' }.build();
let e = &e as &dyn std::error::Error;

assert_eq!(Some('x'), e.request_value::<char>());
```

#### `provide(priority, ...`

[provide-flag-priority]: #providepriority-

The [Provider API][] works by types and can only return one piece of
data for a type. When there are multiple pieces of data for the same
type, the one that is provided *first* will be used.

By default, SNAFU provides data from any source error or
[chained][provide-flag-chain] fields before any data from the current
error. This means that the *deepest* matching data is returned.

Specifying the `priority` flag will cause that data to take precedence
over the chained data, resulting in the *shallower* data being
returned.

```rust,ignore
use snafu::{prelude::*, IntoError};

#[derive(Debug, PartialEq)]
struct Fatal(bool);

#[derive(Debug, Snafu)]
#[snafu(provide(Fatal => Fatal(true)))]
struct InnerError;

#[derive(Debug, Snafu)]
#[snafu(provide(priority, Fatal => Fatal(false)))]
struct PriorityFlagExampleError {
    source: InnerError,
}

let e = PriorityFlagExampleSnafu.into_error(InnerError);
let e = &e as &dyn std::error::Error;

assert_eq!(Some(Fatal(false)), e.request_value::<Fatal>());
```

#### `provide(chain, ...`

[provide-flag-chain]: #providechain-

If a member of your error implements the [Provider API][] and you'd
like for its data to be included when providing data for your error,
but it isn't automatically provided because it's not a source error,
you may add the `chain` flag. This flag must always be combined with
the [`ref` flag][provide-flag-ref].

```rust,ignore
use snafu::prelude::*;
use std::any;

#[derive(Debug)]
struct BlobOfData;

impl any::Provider for BlobOfData {
    fn provide<'a>(&'a self, demand: &mut any::Demand<'a>) {
        demand.provide_value::<u8>(1);
    }
}

#[derive(Debug, Snafu)]
#[snafu(provide(ref, chain, BlobOfData => data))]
struct ChainFlagExampleError {
    data: BlobOfData,
}

let e = ChainFlagExampleSnafu { data: BlobOfData }.build();
let e = &e as &dyn std::error::Error;

assert_eq!(Some(1), e.request_value::<u8>());
```

### API stability concerns

For public errors, it's a good idea to explicitly state your intended
stability guarantees around provided values. Some consumers may expect
that if your error type returns data via the provider API in one
situation, it will continue to do so in future SemVer-compatible
releases. However, doing so can greatly hinder your ability to
refactor your code.

Stating your guarantees is especially useful for opaque errors, which
will expose all the provided data from the inner error type.

[provider API]: https://doc.rust-lang.org/nightly/std/any/index.html#provider-and-demand
[`request_ref`]: https://doc.rust-lang.org/nightly/std/error/trait.Error.html#method.request_ref

## Controlling implicitly generated data

Sometimes, you can capture contextual error data without needing any
arguments. [Backtraces][`Backtrace`] are a common example, but other
global information like the current time or thread ID could also be
useful. In these cases, you can use `#[snafu(implicit)]` on a field
that implements [`GenerateImplicitData`] to remove the need to specify
that data at error construction time:

```rust
use snafu::prelude::*;
use std::time::Instant;

#[derive(Debug, PartialEq)]
struct Timestamp(Instant);

impl snafu::GenerateImplicitData for Timestamp {
    fn generate() -> Self {
        Timestamp(Instant::now())
    }
}

#[derive(Debug, Snafu)]
struct RequestError {
    #[snafu(implicit)]
    timestamp: Timestamp,
}

fn do_request() -> Result<(), RequestError> {
    // ...
    # let request_count = 10;
    ensure!(request_count < 3, RequestSnafu);

    Ok(())
}
```

You can use `#[snafu(implicit(false))]` if a field is incorrectly
automatically identified as containing implicit data.

## Controlling stringly-typed errors

This allows your custom error type to behave like the [`Whatever`][]
error type. Since it is your type, you can implement additional
methods or traits. When placed on a struct or enum variant, you will
be able to use the type with the [`whatever!`][] macro as well as
`whatever_context` methods, such as [`ResultExt::whatever_context`][].

```rust
# use snafu::prelude::*;
#[derive(Debug, Snafu)]
enum Error {
    SpecificError {
        username: String,
    },

    #[snafu(whatever, display("{message}"))]
    GenericError {
        message: String,

        // Having a `source` is optional, but if it is present, it must
        // have this specific attribute and type:
        #[snafu(source(from(Box<dyn std::error::Error>, Some)))]
        source: Option<Box<dyn std::error::Error>>,
    },
}
```

## Controlling how the `snafu` crate is resolved

If the `snafu` crate is not called `snafu` for some reason, you can
use `#[snafu(crate_root)]` to instruct the macro how to find the crate
root:

```rust
# use snafu as my_custom_naming_of_snafu;
use my_custom_naming_of_snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(crate_root(my_custom_naming_of_snafu))]
enum Error {
    SomeFailureMode,
}

#[derive(Debug, Snafu)]
#[snafu(crate_root(my_custom_naming_of_snafu))]
struct ApiError(Error);
```

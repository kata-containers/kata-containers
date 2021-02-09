# The `Fail` trait

The `Fail` trait is a replacement for [`std::error::Error`][stderror]. It has
been designed to support a number of operations:

- Because it is bound by both `Debug` and `Display`, any failure can be
  printed in two ways.
- It has both a `backtrace` and a `cause` method, allowing users to get
  information about how the error occurred.
- It supports wrapping failures in additional contextual information.
- Because it is bound by `Send` and `Sync`, failures can be moved and shared
  between threads easily.
- Because it is bound by `'static`, the abstract `Fail` trait object can be
  downcast into concrete types.

Every new error type in your code should implement `Fail`, so it can be
integrated into the entire system built around this trait. You can manually
implement `Fail` yourself, or you can use the derive for `Fail` defined
in a separate crate and documented [here][derive-docs].

Implementors of this trait are called 'failures'.

## Cause

Often, an error type contains (or could contain) another underlying error type
which represents the "cause" of this error - for example, if your custom error
contains an `io::Error`, that is the cause of your error.

The cause method on the `Fail` trait allows all errors to expose their underlying
cause - if they have one - in a consistent way. Users can loop over the chain
of causes, for example, getting the entire series of causes for an error:

```rust
// Assume err is a type that implements `Fail`
let mut fail: &Fail = err;

while let Some(cause) = fail.cause() {
    println!("{}", cause);

    // Make `fail` the reference to the cause of the previous fail, making the
    // loop "dig deeper" into the cause chain.
    fail = cause;
}
```

Because `&Fail` supports downcasting, you can also inspect causes in more
detail if you are expecting a certain failure:

```rust
while let Some(cause) = fail.cause() {

    if let Some(err) = cause.downcast_ref::<io::Error>() {
        // treat io::Error specially
    } else {
        // fallback case
    }

    fail = cause;
}
```

For convenience an iterator is also provided:

```rust
// Assume err is a type that implements `Fail`
let mut fail: &Fail = err;

for cause in fail.iter_causes() {
    println!("{}", cause);
}
```

## Backtraces

Errors can also generate a backtrace when they are constructed, helping you
determine the place the error was generated and the function chain that called into
that. Like causes, this is entirely optional - the authors of each failure
have to decide if generating a backtrace is appropriate in their use case.

The backtrace method allows all errors to expose their backtrace if they have
one. This enables a consistent method for getting the backtrace from an error:

```rust
// We don't even know the type of the cause, but we can still get its
// backtrace.
if let Some(bt) = err.cause().and_then(|cause| cause.backtrace()) {
    println!("{}", bt)
}
```

The `Backtrace` type exposed by `failure` is different from the `Backtrace` exposed
by the [backtrace crate][backtrace-crate], in that it has several optimizations:

- It has a `no_std` compatible form which will never be generated (because
  backtraces require heap allocation), and should be entirely compiled out.
- It will not be generated unless the `RUST_BACKTRACE` environment variable has
  been set at runtime.
- Symbol resolution is delayed until the backtrace is actually printed, because
  this is the most expensive part of generating a backtrace.

## Context

Often, the libraries you are using will present error messages that don't
provide very helpful information about what exactly has gone wrong. For
example, if an `io::Error` says that an entity was "Not Found," that doesn't
communicate much about what specific file was missing - if it even was a file
(as opposed to a directory for example).

You can inject additional context to be carried with this error value,
providing semantic information about the nature of the error appropriate to the
level of abstraction that the code you are writing operates at. The `context`
method on `Fail` takes any displayable value (such as a string) to act as
context for this error.

Using the `ResultExt` trait, you can also get `context` as a convenient method on
`Result` directly. For example, suppose that your code attempted to read from a
Cargo.toml. You can wrap the `io::Error`s that occur with additional context
about what operation has failed:

```rust
use failure::ResultExt;

let mut file = File::open(cargo_toml_path).context("Missing Cargo.toml")?;
file.read_to_end(&buffer).context("Could not read Cargo.toml")?;
```

The `Context` object also has a constructor that does not take an underlying
error, allowing you to create ad hoc Context errors alongside those created by
applying the `context` method to an underlying error.

## Backwards compatibility

We've taken several steps to make transitioning from `std::error` to `failure` as
painless as possible.

First, there is a blanket implementation of `Fail` for all types that implement
`std::error::Error`, as long as they are `Send + Sync + 'static`. If you are
dealing with a library that hasn't shifted to `Fail`, it is automatically
compatible with `failure` already.

Second, `Fail` contains a method called `compat`, which produces a type that
implements `std::error::Error`. If you have a type that implements `Fail`, but
not the older `Error` trait, you can call `compat` to get a type that does
implement that trait (for example, if you need to return a `Box<Error>`).

The biggest hole in our backwards compatibility story is that you cannot
implement `std::error::Error` and also override the backtrace and cause methods
on `Fail`. We intend to enable this with specialization when it becomes stable.

[derive-docs]: ./derive-fail.html
[stderror]: https://doc.rust-lang.org/std/error/trait.Error.html
[backtrace-crate]: http://alexcrichton.com/backtrace-rs

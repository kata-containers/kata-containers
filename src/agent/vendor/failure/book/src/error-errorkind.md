# An Error and ErrorKind pair

This pattern is the most robust way to manage errors - and also the most high
maintenance. It combines some of the advantages of the [using Error][use-error]
pattern and the [custom failure][custom-fail] patterns, while avoiding some of
the disadvantages each of those patterns has:

1. Like `Error`, this is forward compatible with new underlying kinds of
errors from your dependencies.
2. Like custom failures, this pattern allows you to specify additional information about the error that your dependencies don't give you.
3. Like `Error`, it can be easier to convert underlying errors from dependency
into this type than for custom failures.
4. Like custom failures, users can gain some information about the error
without downcasting.

The pattern is to create two new failure types: an `Error` and an `ErrorKind`,
and to leverage [the `Context` type][context-api] provided by failure.

```rust
#[derive(Debug)]
struct MyError {
    inner: Context<MyErrorKind>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
enum MyErrorKind {
    // A plain enum with no data in any of its variants
    //
    // For example:
    #[fail(display = "A contextual error message.")]
    OneVariant,
    // ...
}
```

Unfortunately, it is not easy to correctly derive `Fail` for `MyError` so that
it delegates things to its inner `Context`. You should write those impls
yourself:

```rust
impl Fail for MyError {
    fn name(&self) -> Option<&str> {
        self.inner.name()
    }

    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for MyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}
```

You should also provide some conversions and accessors, to go between a
Context, your ErrorKind, and your Error:

```rust
impl MyError {
    pub fn kind(&self) -> MyErrorKind {
        *self.inner.get_context()
    }
}

impl From<MyErrorKind> for MyError {
    fn from(kind: MyErrorKind) -> MyError {
        MyError { inner: Context::new(kind) }
    }
}

impl From<Context<MyErrorKind>> for MyError {
    fn from(inner: Context<MyErrorKind>) -> MyError {
        MyError { inner: inner }
    }
}
```

With this code set up, you can use the context method from failure to apply
your ErrorKind to `Result`s in underlying libraries:

```rust
use failure::ResultExt;
perform_some_io().context(ErrorKind::NetworkFailure)?;
```

You can also directly throw `ErrorKind` without an underlying error when
appropriate:

```rust
Err(ErrorKind::DomainSpecificError)?
```

### What should your ErrorKind contain?

Your error kind probably should not carry data - and if it does, it should only
carry stateless data types that provide additional information about what the
`ErrorKind` means. This way, your `ErrorKind` can be `Eq`, making it
easy to use as a way of comparing errors.

Your ErrorKind is a way of providing information about what errors mean
appropriate to the level of abstraction that your library operates at. As some
examples:

- If your library expects to read from the user's `Cargo.toml`, you might have
  a `InvalidCargoToml` variant, to capture what `io::Error` and `toml::Error`
  mean in the context of your library.
- If your library does both file system activity and network activity, you
  might have `Filesystem` and `Network` variants, to divide up the `io::Error`s
  between which system in particular failed.

Exactly what semantic information is appropriate depends entirely on what this
bit of code is intended to do.

## When might you use this pattern?

The most likely use cases for this pattern are mid-layer which perform a
function that requires many dependencies, and that are intended to be used in
production. Libraries with few dependencies do not need to manage many
underlying error types and can probably suffice with a simpler [custom
failure][custom-fail]. Applications that know they are almost always just going
to log these errors can get away with [using the Error type][use-error] rather
than managing extra context information.

That said, when you need to provide the most expressive information about an
error possible, this can be a good approach.

## Caveats on this pattern

This pattern is the most involved pattern documented in this book. It involves
a lot of boilerplate to set up (which may be automated away eventually), and it
requires you to apply a contextual message to every underlying error that is
thrown inside your code. It can be a lot of work to maintain this pattern.

Additionally, like the Error type, the Context type may use an allocation and a
dynamic dispatch internally. If you know this is too expensive for your use
case, you should not use this pattern.

[use-error]: ./use-error.html
[custom-fail]: ./custom-fail.html
[context-api]: https://docs.rs/failure/latest/failure/struct.Context.html

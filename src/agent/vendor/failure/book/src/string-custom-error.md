# Strings and custom fail type

This pattern is an hybrid between the [_An Error and ErrorKind pair_](./error-errorkind.md) and
[_Using the Error type_](./use-error.md).

Such an error type can be implemented in the same way that what was shown in
the [_An Error and ErrorKind pair_](./error-errorkind.md) pattern, but here, the context is a
simple string:

```rust
extern crate core;
extern crate failure;

use core::fmt::{self, Display};
use failure::{Backtrace, Context, Fail, ResultExt};

#[derive(Debug)]
pub struct MyError {
    inner: Context<String>,
}

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

To make the type easier to use, a few impls can be added:

```rust
// Allows writing `MyError::from("oops"))?`
impl From<&'static str> for MyError {
    fn from(msg: &'static str) -> MyError {
        MyError {
            inner: Context::new(msg),
        }
    }
}

// Allows adding more context via a String
impl From<Context<String>> for MyError {
    fn from(inner: Context<String>) -> MyError {
        MyError { inner }
    }
}

// Allows adding more context via a &str
impl From<Context<&'static str>> for MyError {
    fn from(inner: Context<&'static str>) -> MyError {
        MyError {
            inner: inner.map(|s| s.to_string()),
        }
    }
}
```

Here is how it is used:

```rust
fn main() {
    println!("{:?}", err2());
}

// Unlike the "Using the Error type" pattern, functions return our own error
// type here.
fn err1() -> Result<(), MyError> {
    Ok(Err(MyError::from("err1"))?)
}

fn err2() -> Result<(), MyError> {
    // Unlike the "An Error and ErrorKind pair" pattern, our context is a
    // simple string. We can chain errors and provide detailed error messages,
    // but we don't have to deal with the complexity of an error kind type
    Ok(err1().context("err2")?)
}
```

## Variant with `&'static str`

If you don't need to format strings, you can avoid an
allocation by using a `Context<&'static str>` instead of a
`Context<String>`.

```rust
extern crate core;
extern crate failure;

use core::fmt::{self, Display};
use failure::{Backtrace, Context, Fail, ResultExt};

#[derive(Debug)]
pub struct MyError {
    inner: Context<&'static str>,
}

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

impl From<&'static str> for MyError {
    fn from(msg: &'static str) -> MyError {
        MyError {
            inner: Context::new(msg.into()),
        }
    }
}

impl From<Context<&'static str>> for MyError {
    fn from(inner: Context<&'static str>) -> MyError {
        MyError {
            inner,
        }
    }
}
```

## When might you use this pattern?

Sometimes, you don't want to use the [_Using the Error type_](./use-error.md)
pattern, because you want to expose a few different error types. But you don't
want to use the [_An Error and ErrorKind pair_](./error-errorkind.md) pattern
either, because there is no need to provide the context as an enum or because
it would be too much work, if the error can occur in many different contexts.

For instance, if you're writing a library that decodes/encodes a complex binary
format, you might want to expose a `DecodeError` and an `EncodeError` error
type, but provide the context as a simple string instead of an error kind, because:

- users may not care too much about the context in which a `DecodeError` or
  `EncodeError` was encountered, they just want a nice message to explain it
- your binary format is really complex, errors can occur in many different
  places, and you don't want to end up with a giant `ErrorKind` enum


## Caveats on this pattern

If using the `Context<String>` variant, an extra allocation is used for the string.

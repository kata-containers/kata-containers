# A Custom Fail type

This pattern is a way to define a new kind of failure. Defining a new kind of
failure can be an effective way of representing an error for which you control
all of the possible failure cases. It has several advantages:

1. You can enumerate exactly all of the possible failures that can occur in
this context.
2. You have total control over the representation of the failure type.
3. Callers can destructure your error without any sort of downcasting.

To implement this pattern, you should define your own type that implements
`Fail`. You can use the [custom derive][derive-fail] to make this easier. For
example:

```rust
#[derive(Fail, Debug)]
#[fail(display = "Input was invalid UTF-8")]
pub struct Utf8Error;
```

This type can become as large and complicated as is appropriate to your use
case. It can be an enum with a different variant for each possible error, and
it can carry data with more precise information about the error. For example:

```rust
#[derive(Fail, Debug)]
#[fail(display = "Input was invalid UTF-8 at index {}", index)]
pub struct Utf8Error {
    index: usize,
}
```

## When might you use this pattern?

If you need to raise an error that doesn't come from one of your dependencies,
this is a great pattern to use.

You can also use this pattern in conjunction with [using `Error`][use-error] or
defining an [Error and ErrorKind pair][error-errorkind]. Those functions which
are "pure logic" and have a very constrained set of errors (such as parsing
simple formats) might each return a different custom Fail type, and then the
function which merges them all together, does IO, and so on, would return a
more complex type like `Error` or your custom Error/ErrorKind.

## Caveats on this pattern

When you have a dependency which returns a different error type, often you will
be inclined to add it as a variant on your own error type. When you do that,
you should tag the underlying error as the `#[fail(cause)]` of your error:

```rust
#[derive(Fail, Debug)]
pub enum MyError {
    #[fail(display = "Input was invalid UTF-8 at index {}", _0)]
    Utf8Error(usize),
    #[fail(display = "{}", _0)]
    Io(#[fail(cause)] io::Error),
}
```

Up to a limit, this design can work. However, it has some problems:

- It can be hard to be forward compatible with new dependencies that raise
  their own kinds of errors in the future.
- It defines a 1-1 relationship between a variant of the error and an
  underlying error.

Depending on your use case, as your function grows in complexity, it can be
better to transition to [using Error][use-error] or [defining an Error &
ErrorKind pair][error-errorkind].

[derive-fail]: ./derive-fail.html
[use-error]: ./use-error.html
[error-errorkind]: ./error-errorkind.html

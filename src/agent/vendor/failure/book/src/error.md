# The `Error` type

In addition to the trait `Fail`, failure provides a type called `Error`. Any
type that implements `Fail` can be cast into `Error` using From and Into, which
allows users to throw errors using `?` which have different types, if the
function returns an `Error`.

For example:

```rust
// Something you can deserialize
#[derive(Deserialize)]
struct Object {
    ...
}

impl Object {
    // This throws both IO Errors and JSON Errors, but they both get converted
    // into the Error type.
    fn from_file(path: &Path) -> Result<Object, Error> {
        let mut string = String::new();
        File::open(path)?.read_to_string(&mut string)?;
        let object = json::from_str(&string)?;
        Ok(object)
    }
}
```

## Causes and Backtraces

The Error type has all of the methods from the Fail trait, with a few notable
differences. Most importantly, the cause and backtrace methods on Error do not
return Options - an Error is *guaranteed* to have a cause and a backtrace.

```rust
// Both methods are guaranteed to return an &Fail and an &Backtrace
println!("{}, {}", error.cause(), error.backtrace())
```

An `Error`'s cause is always the failure that was cast into this `Error`.
That failure may have further underlying causes. Unlike Fail, this means that
the cause of an Error will have the same Display representation as the Error
itself.

As to the error's guaranteed backtrace, when the conversion into the Error type
happens, if the underlying failure does not provide a backtrace, a new
backtrace is constructed pointing to that conversion point (rather than the
origin of the error). This construction only happens if there is no underlying
backtrace; if it does have a backtrace no new backtrace is constructed.

## Downcasting

The Error type also supports downcasting into any concrete Fail type. It can be
downcast by reference or by value - when downcasting by value, the return type
is `Result<T, Error>`, allowing you to get the error back out of it.

```rust
match error.downcast::<io::Error>() {
    Ok(io_error)    => { ... }
    Err(error)      => { ... }
}
```

## Implementation details

`Error` is essentially a trait object, but with some fanciness it may generate
and store the backtrace if the underlying failure did not have one. In
particular, we use a custom dynamically sized type to store the backtrace
information inline with the trait object data.

```rust
struct Error {
    // Inner<Fail> is a dynamically sized type
    inner: Box<Inner<Fail>>,
}

struct Inner<F: Fail> {
    backtrace: Backtrace,
    failure: F,
}
```

By storing the backtrace in the heap this way, we avoid increasing the size of
the Error type beyond that of two non-nullable pointers. This keeps the size of
the `Result` type from getting too large, avoiding having a negative impact on
the "happy path" of returning Ok. For example, a `Result<(), Error>` should be
represented as a pair of nullable pointers, with the null case representing
`Ok`. Similar optimizations can be applied to values up to at least a pointer
in size.

To emphasize: Error is intended for use cases where the error case is
considered relatively uncommon. This optimization makes the overhead of an
error less than it otherwise would be for the Ok branch. In cases where errors
are going to be returned extremely frequently, returning this Error type is
probably not appropriate, but you should benchmark in those cases.

(As a rule of thumb: if you're not sure if you can afford to have a trait
object, you probably *can* afford it. Heap allocations are not nearly as cheap
as stack allocations, but they're cheap enough that you can almost always
afford them.)

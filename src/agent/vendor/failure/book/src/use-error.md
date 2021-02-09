# Use the `Error` type

This pattern is a way to manage errors when you have multiple kinds of failure
that could occur during a single function. It has several distinct advantages:

1. You can start using it without defining any of your own failure types.
2. All types that implement `Fail` can be thrown into the `Error` type using
the `?` operator.
3. As you start adding new dependencies with their own failure types, you can
start throwing them without making a breaking change.

To use this pattern, all you need to do is return `Result<_, Error>` from your
functions:

```rust
use std::io;
use std::io::BufRead;

use failure::Error;
use failure::err_msg;

fn my_function() -> Result<(), Error> {
    let stdin = io::stdin();

    for line in stdin.lock().lines() {
        let line = line?;

        if line.chars().all(|c| c.is_whitespace()) {
            break
        }

        if !line.starts_with("$") {
            return Err(format_err!("Input did not begin with `$`"));
        }

        println!("{}", &line[1..]);
    }

    Ok(())
}
```

## When might you use this pattern?

This pattern is very effective when you know you will usually not need to
destructure the error this function returns. For example:

- When prototyping.
- When you know you are going to log this error, or display it to the user,
  either all of the time or nearly all of the time.
- When it would be impractical for this API to report more custom context for
  the error (e.g. because it is a trait that doesn't want to add a new Error
  associated type).

## Caveats on this pattern

There are two primary downsides to this pattern:

- The `Error` type allocates. There are cases where this would be too
  expensive. In those cases you should use a [custom failure][custom-fail].
- You cannot recover more information about this error without downcasting. If
  your API needs to express more contextual information about the error, use
  the [Error and ErrorKind][error-errorkind] pattern.

[custom-fail]: ./custom-fail.html
[error-errorkind]: ./error-errorkind.html

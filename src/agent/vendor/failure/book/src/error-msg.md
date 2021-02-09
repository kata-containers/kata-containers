# Strings as errors

This pattern is a way to create new errors without doing much set up. It is
definitely the sloppiest way to throw errors. It can be great to use this
during prototyping, but maybe not in the final product.

String types do not implement `Fail`, which is why there are two adapters to
create failures from a string:

- [`failure::err_msg`][err-msg-api] - a function that takes a displayable
  type and creates a failure from it. This can take a String or a string
  literal.
- [`format_err!`][format-err-api] - a macro with string interpolation, similar
  to `format!` or `println!`.

```rust
fn check_range(x: usize, range: Range<usize>) -> Result<usize, Error> {
    if x < range.start {
        return Err(format_err!("{} is below {}", x, range.start));
    }
    if x >= range.end {
        return Err(format_err!("{} is above {}", x, range.end));
    }
    Ok(x)
}
```

If you're going to use strings as errors, we recommend [using
`Error`][use-error] as your error type, rather than `ErrorMessage`; this way,
if some of your strings are `String` and some are `&'static str`, you don't
need worry about merging them into a single string type.

## When might you use this pattern?

This pattern is the easiest to set up and get going with, so it can be great
when prototyping or spiking out an early design. It can also be great when you
know that an error variant is extremely uncommon, and that there is really no
way to handle it other than to log the error and move on.

## Caveats on this pattern

If you are writing a library you plan to publish to crates.io, this is probably
not a good way to handle errors, because it doesn't give your clients very much
control. For public, open source libraries, we'd recommend using [custom
failures][custom-fail] in the cases where you would use a string as an error.

This pattern can also be very brittle. If you ever want to branch over which
error was returned, you would have to match on the exact contents of the
string. If you ever change the string contents, that will silently break that
match.

For these reasons, we strongly recommend against using this pattern except for
prototyping and when you know the error is just going to get logged or reported
to the users.

[custom-fail]: ./custom-fail.html
[use-error]: ./use-error.html
[err-msg-api]: https://docs.rs/failure/latest/failure/fn.err_msg.html
[format-err-api]: https://docs.rs/failure/latest/failure/macro.format_err.html

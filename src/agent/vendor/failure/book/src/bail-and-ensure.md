# `bail!` and `ensure!`

If you were a fan of the `bail!` and ensure! macros from error-chain, good news. failure has a version of these macros as well.

The `bail!` macro returns an error immediately, based on a format string. The `ensure!` macro additionally takes a conditional, and returns the error only if that conditional is false. You can think of `bail!` and `ensure!` as being analogous to `panic!` and `assert!`, but throwing errors instead of panicking.

`bail!` and `ensure!` macros are useful when you are prototyping and you want to write your custom errors later. It is also the simplest example of using the failure crate.

## Example
```rust
#[macro_use] extern crate failure;

fn safe_cast_to_unsigned(n:i32) -> Result<u32, error::Failure>
{
    ensure!(n>=0, "number cannot be smaller than 0!");
    (u32) n
}
```
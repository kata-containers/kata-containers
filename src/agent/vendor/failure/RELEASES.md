# Version 0.1.6

- Update `syn`, `quote`, and `proc_macro2` dependencies to 1.0.
- Bump MSRV to 1.31.0.

# Version 0.1.5

- Resolve a regression with error conversions (#290)
- Added `name()` to `Fail` and `Error`

# Version 0.1.4

- Improved error reporting of the derive feature
- Resolved a potential internal ambiguity when using the backtrace feature
  that prevented backtrace from improving an upstream API.
- Changed the bounds on std error compat conversions through the From trait
  to take Sync and Send into account.

# Version 0.1.3

- Added `Context::map`
- Fixed a memory leak for older rust versions on error downcast

# Version 0.1.2

The original plan to release 1.0.0 was changed so that version 0.1.1 is released and a related [RFC to fix the error trait](https://github.com/rust-lang/rfcs/pull/2504) is submitted. See README for details.

- Fix `failure_derive` to work with Rust 2018.
- Add `#[fail(cause)]` that works similarly with `#[cause]`. The new form is preferred.
- Fix `"backtrace"` feature to work without `"std"` feature.
- Add `Compat::get_ref`.
- Add `Fallible`.
- Deprecate `Fail::causes` and `<dyn Fail>::causes` in favor of newly added `<dyn Fail>::iter_causes`.
- Deprecate `Fail::root_cause` and `<dyn Fail>::root_cause` in favor of newly added `<dyn Fail>::find_root_cause`.
- Add `<dyn Fail>::iter_chain`.
- Implement `Box<Fail>: Fail`.
- Add `Error::from_boxed_compat`.
- Deprecate `Error::cause` in favor of newly added `Error::as_fail`.
- Deprecate `Error::causes` in favor of newly added `Error::iter_chain`.
- Deprecate `Error::root_cause` in favor of newly added `Error::find_root_cause`.
- Add `Error::iter_causes`.
- Implement `Error: AsRef<Fail>`.
- Fix `Debug` implementation of `SyncFailure`.

# Version 0.1.1

- Add a `Causes` iterator, which iterates over the causes of a failure. Can be
  accessed through the `Fail::causes` or `Error::causes` methods.
- Add the `bail!` macro, which "throws" from the function.
- Add the `ensure!` macro, which is like an "assert" which throws instead of
  panicking.
- The derive now supports a no_std mode.
- The derive is re-exported from `failure` by default, so that users do not
  have to directly depend on `failure_derive`.
- Add a impl of `From<D> for Context<D>`, allowing users to `?` the `D` type to
  produce a `Context<D>` (for cases where there is no further underlying
  error).

# Version 0.1.0

- Initial version.

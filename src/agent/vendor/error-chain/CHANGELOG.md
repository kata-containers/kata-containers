# 0.12.4
- [executable bits to support build platform linters](https://github.com/rust-lang-nursery/error-chain/pull/289)

# 0.12.2
- [allow `Error::description` to be used for rust below 1.42](https://github.com/rust-lang-nursery/error-chain/pull/285)
- [Improvements to has_backtrace_depending_on_env](https://github.com/rust-lang-nursery/error-chain/pull/277)
- Backtrace support now requires rust 1.32.0

# 0.12.1

- [`std::error::Error::cause` deprecation update](https://github.com/rust-lang-nursery/error-chain/pull/255)
- [Macro invocations use 2018 style](https://github.com/rust-lang-nursery/error-chain/pull/253)

# 0.12.0

- [Remove `impl Deref<Kind> for Error`](https://github.com/rust-lang-nursery/error-chain/pull/192)
- [Fix warning](https://github.com/rust-lang-nursery/error-chain/pull/247)

# 0.11.0

- Change last rust version supported to 1.14
- [Cache whether RUST_BACKTRACE is enabled in a relaxed atomic static.](https://github.com/rust-lang-nursery/error-chain/pull/210)
- [Mask the `quick_error` macro from the doc](https://github.com/rust-lang-nursery/error-chain/pull/210)
- [Make generated `ErrorKind` enums non-exhaustive](https://github.com/rust-lang-nursery/error-chain/pull/193)
- All 0.11.0-rc.2 changes

# 0.11.0-rc.2

- [Make `ErrorChainIter`'s field private](https://github.com/rust-lang-nursery/error-chain/issues/178)
- [Rename `ErrorChainIter` to `Iter`](https://github.com/rust-lang-nursery/error-chain/issues/168)
- [Implement `Debug` for `ErrorChainIter`](https://github.com/rust-lang-nursery/error-chain/issues/169)
- [Rename `ChainedError::display` to `display_chain`](https://github.com/rust-lang-nursery/error-chain/issues/180)
- [Add a new method for `Error`: `chain_err`.](https://github.com/rust-lang-nursery/error-chain/pull/141)
- [Allow `chain_err` to be used on `Option<T>`](https://github.com/rust-lang-nursery/error-chain/pull/156)
- [Add support for creating an error chain on boxed trait errors (`Box<Error>`)](https://github.com/rust-lang-nursery/error-chain/pull/156)
- [Remove lint for unused doc comment.](https://github.com/rust-lang-nursery/error-chain/pull/199)
- [Hide error_chain_processed macro from documentation.](https://github.com/rust-lang-nursery/error-chain/pull/212)

# 0.10.0

- [Add a new constructor for `Error`: `with_chain`.](https://github.com/rust-lang-nursery/error-chain/pull/126)
- [Add the `ensure!` macro.](https://github.com/rust-lang-nursery/error-chain/pull/135)

# 0.9.0

- Revert [Add a `Sync` bound to errors](https://github.com/rust-lang-nursery/error-chain/pull/110)

# 0.8.1

- Add crates.io category.

# 0.8.0

- [Add a `Sync` bound to errors](https://github.com/rust-lang-nursery/error-chain/pull/110)
- [Add `ChainedError::display` to format error chains](https://github.com/rust-lang-nursery/error-chain/pull/113)

# 0.7.2

- Add `quick_main!` (#88).
- `allow(unused)` for the `Result` wrapper.
- Minimum rust version supported is now 1.10 on some conditions (#103).

# 0.7.1

- [Add the `bail!` macro](https://github.com/rust-lang-nursery/error-chain/pull/76)

# 0.7.0

- [Rollback several design changes to fix regressions](https://github.com/rust-lang-nursery/error-chain/pull/75)
- New `Variant(Error) #[attrs]` for `links` and `foreign_links`.
- Hide implementation details from the doc.
- Always generate `Error::backtrace`.

# 0.6.2

- Allow dead code.

# 0.6.1

- Fix wrong trait constraint in ResultExt implementation (#66).

# 0.6.0

- Conditional compilation for error variants.
- Backtrace generation is now a feature.
- More standard trait implementations for extra convenience.
- Remove ChainErr.
- Remove need to specify `ErrorKind` in `links {}`.
- Add ResultExt trait.
- Error.1 is a struct instead of a tuple.
- Error is now a struct.
- The declarations order is more flexible.
- Way better error reporting when there is a syntax error in the macro call.
- `Result` generation can be disabled.
- At most one declaration of each type can be present.

# 0.5.0

- [Only generate backtraces with RUST_BACKTRACE set](https://github.com/rust-lang-nursery/error-chain/pull/27)
- [Fixup matching, disallow repeating "types" section](https://github.com/rust-lang-nursery/error-chain/pull/26)
- [Fix tests on stable/beta](https://github.com/rust-lang-nursery/error-chain/pull/28)
- [Only deploy docs when tagged](https://github.com/rust-lang-nursery/error-chain/pull/30)

Contributors: benaryorg, Brian Anderson, Georg Brandl

# 0.4.2

- [Fix the resolution of the ErrorKind description method](https://github.com/rust-lang-nursery/error-chain/pull/24)

Contributors: Brian Anderson

# 0.4.1 (yanked)

- [Fix a problem with resolving methods of the standard Error type](https://github.com/rust-lang-nursery/error-chain/pull/22)

Contributors: Brian Anderson

# 0.4.0 (yanked)

- [Remove the foreign link description and forward to the foreign error](https://github.com/rust-lang-nursery/error-chain/pull/19)
- [Allow missing sections](https://github.com/rust-lang-nursery/error-chain/pull/17)

Contributors: Brian Anderson, Taylor Cramer

# 0.3.0

- [Forward Display implementation for foreign errors](https://github.com/rust-lang-nursery/error-chain/pull/13)

Contributors: Brian Anderson, Taylor Cramer

# 0.2.2

- [Don't require `types` section in macro invocation](https://github.com/rust-lang-nursery/error-chain/pull/8)
- [Add "quick start" to README](https://github.com/rust-lang-nursery/error-chain/pull/9)

Contributors: Brian Anderson, Jake Shadle, Nate Mara

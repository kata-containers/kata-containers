# Changes for `nonzero_ext`

<!-- next-header -->

## [Unreleased] - ReleaseDate

## [0.3.0] - 2021-08-21

### Changed
* The `nonzero!` macro now enforces that its arguments are integer
  literals. Any other non-zeroable types (even if they implement
  `nonzero_ext::NonZeroAble`) can not be accepted. This fixes
  [#17](https://github.com/antifuchs/nonzero_ext/issues/17).

### Contributors
* [`@joshlf`](https://github.com/joshlf)
* [`@ComputerDruid`](https://github.com/ComputerDruid)

## [0.2.0] - 2019-12-23

### Added
* Support for `NonZeroI*` types - now `nonzero_ext` should include support for all non-zero integer types that the
 standard library exports.
* Support for using `nonzero!` in a [const context](https://doc.rust-lang.org/reference/const_eval.html).
* This [CHANGELOG](./CHANGELOG.md) file. I have tried to backfill the major changes since initial release, but there
 are bound to be gaps.

### Changed
* The `as_nonzero` and `as_nonzero_unchecked` methods on the `NonZeroAble` trait are now named `into_nonzero` and
 `into_nonzero_unchecked`, respectively. The old methods still exist, but are deprecated.

## [v0.1.3] - 2019-03-10

### Added
* Ability to use the `nonzero_ext` crate in `no_std` mode; to use it without the `std` library, disable default
 features when pulling this crate into your project.

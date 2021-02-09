# Changelog

All notable changes to this project will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org).

## [Unreleased]

## [0.1.11] - 2020-10-20

* Suppress `clippy::redundant_pub_crate` lint in generated code.

* Documentation improvements.

## [0.1.10] - 2020-10-01

* Suppress `drop_bounds` lint, which will be added to rustc in the future. See [taiki-e/pin-project#272](https://github.com/taiki-e/pin-project/issues/272) for more details.

## [0.1.9] - 2020-09-29

* Fix trailing comma support in generics

## [0.1.8] - 2020-09-26

* Fix compatibility of generated code with `forbid(future_incompatible)`

  Note: This does not guarantee compatibility with `forbid(future_incompatible)` in the future.
  If rustc adds a new lint, we may not be able to keep this.

## [0.1.7] - 2020-06-04

* [Support `?Sized` bounds in where clauses.][22]

* [Fix lifetime inference error when an associated type is used in fields.][20]

* Suppress `clippy::used_underscore_binding` lint in generated code.

* Documentation improvements.

[20]: https://github.com/taiki-e/pin-project-lite/pull/20
[22]: https://github.com/taiki-e/pin-project-lite/pull/22

## [0.1.6] - 2020-05-31

* [Support lifetime bounds in where clauses.][18]

* Documentation improvements.

[18]: https://github.com/taiki-e/pin-project-lite/pull/18

## [0.1.5] - 2020-05-07

* [Support overwriting the name of core crate.][14]

[14]: https://github.com/taiki-e/pin-project-lite/pull/14

## [0.1.4] - 2020-01-20

* [Support ?Sized bounds in generic parameters.][9]

[9]: https://github.com/taiki-e/pin-project-lite/pull/9

## [0.1.3] - 2020-01-20

* [Support lifetime bounds in generic parameters.][7]

[7]: https://github.com/taiki-e/pin-project-lite/pull/7

## [0.1.2] - 2020-01-05

* [Support recognizing default generic parameters.][6]

[6]: https://github.com/taiki-e/pin-project-lite/pull/6

## [0.1.1] - 2019-11-15

* [`pin_project!` macro now determines the visibility of the projection type/method is based on the original type.][5]

[5]: https://github.com/taiki-e/pin-project-lite/pull/5

## [0.1.0] - 2019-10-22

Initial release

[Unreleased]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.11...HEAD
[0.1.11]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.10...v0.1.11
[0.1.10]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/taiki-e/pin-project-lite/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/taiki-e/pin-project-lite/releases/tag/v0.1.0

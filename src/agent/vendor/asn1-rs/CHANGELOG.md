# Change Log

## [Unreleased][unreleased]

### Changed/Fixed

### Added

### Thanks

## 0.5.1

Minor fixes:

- Fix constraints too strict on `TaggedValue::FromDer`, do not auto-derive
- Update oid-registry
- Fix `Any::as_relative_oid` to take a reference (and not consume input)

derive:

- Add special case handler for alias to Any
- Add support for DEFAULT attribute

## 0.5.0

This release adds some new methods and custom derive attributes.
It also adds a lot of tests to improve code coverage.

asn1-rs:

- Add helper types for Application/Private tagged values
- Any: add methods `from_ber_and_then` (and `_der`)
- TaggedParser: add documentation for `from_ber_and_then` (and `_der`)
- Oid: add method `starts_with`
- Fix documentation of application and private tagged helpers
- Fix clippy warnings

derive:

- Add custom derive BerAlias and DerAlias

coverage:

- Add many tests to improve coverage

## 0.4.2

Bugfix release:
- Remove explicit output lifetime in traits
- Fix wrong encoding `BmpString` when using `ToDer`
- Fix parsing of some EmbeddedPdv subtypes
- Fix encoded length for Enumerated
- Add missing `DerAutoDerive` impl for bool
- Add missing `DerAutoDerive` impl for f32/f64
- Remove redundant check, `Any::from_der` checks than length is definite
- Length: fix potential bug when adding Length + Indefinite
- Fix inverted logic in `Header::assert_definite()`

## 0.4.1

Minor fix:
- add missing file in distribution (fix docs.rs build)

## 0.4.0

asn1-rs:

- Add generic error parameter in traits and in types
  - This was added for all types except a few (like `Vec<T>` or `BTreeSet<T>`) due to
    Rust compiler limitations
- Add `DerAutoDerive` trait to control manual/automatic implementation of `FromDer`
  - This allow controlling automatic trait implementation, and providing manual
    implementations of both `FromDer` and `CheckDerConstraints`
- UtcTime: Introduce utc_adjusted_date() to map 2 chars years date to 20/21 centuries date (#9)

derive:

- Add attributes to simplify deriving EXPLICIT, IMPLICIT and OPTIONAL
- Add support for different tag classes (like APPLICATION or PRIVATE)
- Add support for custom errors and mapping errors
- Add support for deriving BER/DER SET
- DerDerive: derive both CheckDerConstraints and FromDer

documentation:

- Add doc modules for recipes and for custom derive attributes
- Add note on trailing bytes being ignored in sequence
- Improve documentation for notation with braces in TaggedValue
- Improve documentation

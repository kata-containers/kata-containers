# Change Log

## [Unreleased][unreleased]

### Changed/Fixed

### Added

### Thanks

## 8.1.0

### Changed/Fixed

- Upgrade `asn1-rs` to 0.5.0 (new features only: only increment minor number)

## 8.0.0

### Changed/Fixed

- Upgrade `asn1-rs` to 0.4.0
  This causes an increment of the major number, because `asn1-rs` is re-exported

## 7.0.0

This release marks the beginning of the merge with the `asn1-rs` crate. **This will break things.**

However, this is necessary, because the `asn1-rs` crate is much cleaner and supports more types
and features (like serialization, custom derive, etc.).
Ultimately, this crate will become a frontend to `asn1-rs`, that will be optional: crate users can
switch to `asn1-rs` and use it directly.

### Changed/Fixed

MSRV: The minimum supported rust version is now *1.53*.

`BerObjectHeader`:

- `BerSize` has been renamed to `Length`
- `BerClass` has been renamed to `Class`
- `BerTag` has been renamed to `Tag`
- Header fields are now private. Getters/setters have been added, and must be used to access/modify fields

`BerObjectContent`:
- `Unknown` variant now contains an `Any` object, with both the header and object content
- `Private` variant has been merged into `Unknown`
- `BmpString`, `GeneralString`, `GraphicString`, `T61String`, `VideotexString` and `ObjectDescriptor` are now decoded
- `GeneralizedTime` and `UtcTime` are now decoded

`BerError`:

- Add error types `UnexpectedClass` and `UnexpectedTag`
- Store expected and found item in error to help debugging
- Keep `InvalidTag` for tags with invalid form (length/encoding/etc.)
- Use `displaydoc` for `BerError`
- Parsing an indefinite length in DER now raises `IndefiniteLengthUnexpected`
- Error: when a DER constraint fails, store constraint identifier

`DER`:
- `DerClass` and `DerTag` have been deprecated. Use `Class` and `Tag` instead.
- `DerObjectHeader` has been deprecated. Use `Header` instead.

`Oid`:
- The `Oid` object is now the same as `asn1_rs::Oid` (simply reexported)
- Remove dependency on crate `der-oid-macro`

Misc:
- `ber_read_element_content_as` now requires a non-zero `depth`, or it
  will raise a `BerMaxDepth` error (previously, 0 allowed one level of parsing)
- crate `rusticata-macros` is now re-exported (#55)

### Thanks

- @lilyball
- @erikmarkmartin

## 6.0.0

This release has several major changes:
- upgrade to nom 7
- add support for `no_std`
- remove all macros
- update MSRV to 1.48

### Changed/Fixed

- Do not attempt to parse PRIVATE object contents (closes #48)
- BER: raise error if using Indefinite length and not constructed
- Fix `oid!` macro to be independant of `der_parser` crate name and path (#46)
- Simplify `der-oid-macro`, do not depend on `nom`
- Fix `INTEGER` signed/unsigned parsing (#49)
- Change `as_bigint()` and `as_uint()` to return a `Result`
- Remove deprecated functions

### Added

- Added support for `no_std` (#50)
- Make `BerError` Copy + Clone (#51)
- Add feature 'bitvec' for `.as_bitslice()` methods

### Removed

- Remove all macros

### Thanks

- @yoguorui for `no_std` support
- @SergioBenitez for `BerError` traits
- @lilyball for `INTEGER` parsing

## 5.1.0

### Changed/Fixed

- Remove dependency on proc-macro-hack (attempt to fix #36)
- Update pretty_assertions requirement from 0.6 to 0.7
- Update num-bigint to 0.4 (Closes #42)

## 5.0.1

### Changed/Fixed

- Fix typos in the `parse_[ber|der]_[u32|u64]` doc comments
- Add documentation for BerObjectContent variants (#41)
- Fixes for clippy

### Added

## 5.0.0

See changelog entries for 5.0.0-beta1 and -beta2 for changes since 4.1

### Changed/Fixed

The following changes applies since 5.0.0-beta1, and do not affect 4.x

- Fix potential integer underflow in `bytes_to_u64`
- Fix potential stack recursion overflow for indefinite length objects
  (Add maximum depth).
- Fix potential UB in bitstring_to_u64 with large input and many ignored bits
- Fix constructed objects parsing with indefinite length (do not include EOC)
- Constructed objects: use `InvalidTag` everywhere if tag is not expected
- Integer parsing functions now all return `IntegerTooLarge` instead of `MapRes`
- Ensure Indefinite length form is only used in BER constructed objects

### Added

- Add new error `StringInvalidCharset` and update string parsing methods
- Add methods `parse_ber_slice` and `parse_der_slice` to parse an expected Tag and get content as slice

## 5.0.0-beta2

### Changed/Fixed

- Consistency: reorder arguments or function callbacks, always set input slice as first argument
  (`parse_ber_sequence_defined_g`, `parse_ber_container`, `parse_ber_tagged_explicit_g`, ...)
- Make functions `parse_ber_sequence_of_v` and `parse_ber_set_of_v` accept generic error types

### Added

- Add `parse_ber_content2`, owned version of `parse_ber_content`, which can directly be combined
  with `parse_ber_tagged_implicit_g`
- Add methods to parse DER tagged values and containers (with constraints)

## 5.0.0-beta1

### Changed/Fixed

- Upgrade to nom 6
- Switch all parsers to function-based parsers
- Change representation of size (new type `BerSize`) to support BER indefinite lengths
- Rewrite BER/DER parsing macros to use functional parsing combinators
- The constructed bit is now tested for explicit tagged structures
- Some checks (for ex. tags in constructed objects) now return specific errors (`InvalidTag`)
  instead of generic errors (`Verify`)
- Refactor BerObject for parsing of tagged and optional values
- Add method `as_bitslice()` to BerObject
- Remove Copy trait from BerObjectHeader, copy is non-trivial and should be explicit
- Fix the bug that caused OIDs longer than two subidentifiers which started by subidentifiers "0.0" ("itu-t recommenation") not to be decoded correctly
- Implement the `as_u64` and `as_u32` methods for BerObjects with contents of type `BerObjectContent::BitString`.
- Implement the `VideotexString`, `ObjectDescriptor` `GraphicString`, and `VisibleString` string types. (Non-breaking changes)
- Correctly decode `BMPString` as UTF-16 instead of UTF-8 when printing. (Non-breaking change)
- Turn `UTCTime` and `GeneralizedTime` into a `&str` instead of `&[u8]`, as they inherit from `VisibleString` which is a subset of ASCII. (Breaking change)

### Added

- Add combinator `parse_ber_optional`

### Thanks

By alphabetic order of handle:

- `@cccs-sadugas`
- `@nickelc`
- `@p1-mmr`

## 4.1.0

### Added/Changed

- Re-export num-bigint so crate users do not have to import it
- Add function versions to parse BER sequences/sets (#20)
- Add function versions to parse BER tagged objects (#20)
- Add generic error type to structured parsing functions
- Add function to parse a generic BER container object
- Document that trailing bytes from SEQUENCE/SET are ignored
- Deprecate functions `parse_{ber,der}_explicit` (use `_optional`)

## 4.0.2

### Changed/Fixed

- Upgrade dependencies on num-bigint and der-oid-macro

## 4.0.1

### Changed/Fixed

- Add workaround to fix parsing of empty sequence or set

## 4.0.0

**Attention** This is a major release, with several API-breaking changes. See `UPGRADING.md` for instructions.

### Thanks

- Jannik Schürg (oid, string verifications)

### Added

- Add functions `parse_ber_recursive` and `parse_der_recursive`, allowing to specify maximum 
  recursion depth when parsing
- The string types `IA5String`, `NumericString`, `PrintableString` and `UTF8String`
  do now only parse if the characters are valid.
- `as_str()` was added to `BerObjectContent` to obtain a `&str` for the types above.
  `as_slice()` works as before.
- Implement `Error` trait for `BerError`
- Add method to extract raw tag from header
  - `BerObjectHeader` now has a lifetime and a `raw_tag` field
  - `BerObject` now has a `raw_tag` field
  - Implement `PartialEq` manually for `BerObject`: `raw_tag` is compared only if both fields provide it
- Add type `BerClass`
- Start adding serialization support (experimental) using the `serialize` feature

### Changed/Fixed

- Make header part of `BerObject`, remove duplicate fields
- Maximum recursion logic has changed. Instead of providing the current depth, the argument is
  now the maximum possible depth.
- Change the api around `Oid` to achieve zero-copy. The following changed:
  - The `Oid` struct now has a lifetime and uses `Cow` internally.
  - The procedural macro `oid!` was added.
  - `Oid::from` returns a `Result` now.
  - The `Oid` struct now encodes whether the oid is relative or not.
  - The `Debug` implementation now shows whether the oid is relative
    and uses the bigint feature if available.
  - The `Oid::iter` method now returns an `Option`. `Oid::iter_bigint` was
    added.
  - `Hash` is now derived for `Oid`.
- Minimum rust version is now 1.34

## 3.0.3

- Make the pretty-printer function public
- Fix DER datestring sanity check
- CI
  - add rusfmt check
  - add cargo clippy

## 3.0.2

- Add `parse_ber_u32` and `parse_ber_u64` functions
- Fix typo in description

## 3.0.1

- Add crate `BerResult` and `DerResult` types
- Use crate result types, remove uneeded imports
  - Crates using `der-parser` do not need to import `nom` or `rusticata-macros` anymore
  - Result types are aliases, so API is unchanged

## 3.0.0

- Upgrade to nom 5 (breaks API)
- New error types, now all functions use `BerError`

## 2.1.0

- Handle BER/DER tags that are longer than one byte.
- Set edition to 2018

## 2.0.2

- Revert 2.0.1 release, breaks API

## 2.0.1

- Handle BER/DER tags that are longer than one byte.

## 2.0.0

- Refactor code, split BER and DER, check DER constraints
- Add recursion limit for sequences and sets
- Rustfmt
- Documentation
- Remove unused function `ber_read_element_content`

## 1.1.1

- Fix OID parsing, and add support for relative OIDs
- Add FromStr trait for Oid

## 1.1.0

- Use num-bigint over num and upgrade to 0.2

## 1.0.0

- Upgrade to nom 4

## 0.5.5

- Add functions `parse_der_u32` and `parse_der_u64` to quickly parse integers
- Remove `Oid::from_vec`, `Oid::from` does the same
- Enforce constraints on DER booleans

## 0.5.4

- Add `BitStringObject` to wrap BitString objects
- Mark constructed BitStrings as unsupported
- Do not try to parse application-specific data in `parse_der`

## 0.5.3

- Add function `DerObject::as_u64`
- Add function `DerObject::as_oid_val`
- Add `parse_der_struct!` variant to check tag

## 0.5.2

- Add functions to test object class and primitive/constructed state
- Add macro `parse_der_application!`
- Add macro `parse_der_tagged!` to parse `[x] EXPLICIT` or `[x] IMPLICIT` tagged values

## 0.5.1

- Add type GeneralString
- Add macro `parse_der_struct!`

## 0.5.0

- Allow use of crate without extra use statements
- Use constants for u32 errors instead of magical numbers
- Rename `tag_of_der_content()` to `DerObjectContent::tag`
- Rename DerElementxxx structs to have a consistent naming scheme
- Add documentation for parsing DER sequences and sets, and fix wrong return type for sets
- Fix a lot of clippy warnings
- QA: add pragma rules (disable unsafe code, unstable features etc.)
- More documentation
- Switch license to MIT + APLv2

## 0.4.4

- Add macro parse_der_defined_m, to parse a defined sequence or set
  This macro differs from `parse_der_defined` because it allows using macros
- Rename `DerObject::new_int` to `DerObject::from_int_slice`
- Rename `Oid::to_hex` to `Oid::to_string`
- Document more functions

## 0.4.1

- Add new feature 'bigint' to export DER integers
- OID is now a specific type
- Add new types T61String and BmpString
- Fix wrong expected tag in parse_der_set_of

## 0.4.0

- Der Integers are now represented as slices (byte arrays) since they can be larger than u64.

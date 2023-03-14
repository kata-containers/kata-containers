## Upgrading from 6.x to 7.0

### Header refactor

Header names have changed:
- `BerClass` is now `Class`
- `BerSize` is now `Length`
- `BerTag` is now `Tag`
- `BerObjectHeader` is now `Header`

Changing the names should be enough for upgrades.

To eventually ease upgrades, a new module (`der_parser::ber::compat`) has been added, to provide aliases for these types. It must be imported explicitely.

Header fields are now private. Getters/setters have been added, and must be used to access/modify fields. Replace:
- `hdr.len` by `hdr.length()`
- `hdr.class` by `hdr.class()`
- `hdr.tag` by `hdr.tag()`

`structured` has been renamed to `constructed` to match RFC. Since this field is now private, methods `constructed()`/`set_constructed()` must be used instead of raw access.

### DER

`DerClass` and `DerTag` have been deprecated. Use `Class` and `Tag` instead.

`DerObjectHeader` has been deprecated. Use `Header` instead.

## Upgrading from 4.x to 5.0

### BER variants: ContextSpecific, Optional, Tagged

The variant `ContextSpecific` has been removed from `BerObject`, and 2 new variants have been added:
- `Tagged` for explicit tagged objects,
- `Optional` to simplify writing subparsers with only `BerObject`

This is also used to clarify parsing of tagged values, and the API now clearly says if trying to parse an
optional value or not.

### Ber Size

The `len` field of `BerObjectHeader` is now an enum, to represent definite and indefinite lengths.
To get the value, either match the type, or use `try_from` (which will fail if indefinite).

### Struct parsing Macros

Functions and combinators are now the preferred way of parsing constructed objects.

Macros have been upgrading and use the combinators internally. As a consequence, they do not return
a tuple `(BerObjectHeader, T)` but only the built object `T`. The header should be removed from function
signatures, for ex:
```
-fn parse_struct01(i: &[u8]) -> BerResult<(BerObjectHeader,MyStruct)> {
+fn parse_struct01(i: &[u8]) -> BerResult<MyStruct> {
```

The header was usually ignored, so this should simplify most uses of this macro. To get the header,
use `parse_ber_container` directly.

## Upgrading from 3.x to 4.0

### Ber Object and Header

The `class`, `structured` and `tag` fields were duplicated in `BerObject` and the header.
Now, a header is always created and embedded in the BER object, with the following changes:

- To access these fields, use the header: `obj.tag` becomes `obj.header.tag`, etc.
- `BerObject::to_header()` is now deprecated
- The `len` field is now public. However, in some cases it can be 0 (when creating an object, 0 means that serialization will calculate the length)
- As a consequence, `PartialEq` on BER objects and headers compare `len` only if set in both objects

### BER String types verification

Some BER String types (`IA5String`, `NumericString`, `PrintableString` and `UTF8String`) are now
verified, and will now only parse if the characters are valid.

Their types have change from slice to `str` in the `BerObjectContent` enum.

### BerClass

The `class` field of `BerObject` struct now uses the newtype `BerClass`. Use the provided constants
(for ex `BerClass:Universal`). To access the value, just use `class.0`.

### Maximum depth

The `depth` argument of functions (for ex. `ber_read_element_content_as`) has changed, and is now the maximum possible depth while parsing.
Change it (usually from `0`) to a possible limit, for ex `der_parser::ber::MAX_RECURSION`.

### Oid

This is probably the most impacting change.

OID objects have been refactored, and are now zero-copy. This has several consequences:

- `Oid` struct now has a lifetime, which must be propagated to objects using them
  - This makes having globally static structs difficult. Obtaining a `'static` object is possible
    using the `oid` macro. For ex:

```rust
const SOME_STATIC_OID: Oid<'static> = oid!(1.2.456);
```

- Due to limitations of procedural macros  ([rust
  issue](https://github.com/rust-lang/rust/issues/54727)) and constants used in patterns ([rust issue](https://github.com/rust-lang/rust/issues/31434)), the `oid` macro can not directly be used in patterns, also not through constants.
You can do this, though:

```rust
# use der_parser::{oid, oid::Oid};
# let some_oid: Oid<'static> = oid!(1.2.456);
const SOME_OID: Oid<'static> = oid!(1.2.456);
if some_oid == SOME_OID || some_oid == oid!(1.2.456) {
    println!("match");
}

// Alternatively, compare the DER encoded form directly:
const SOME_OID_RAW: &[u8] = &oid!(raw 1.2.456);
match some_oid.bytes() {
    SOME_OID_RAW => println!("match"),
    _ => panic!("no match"),
}
```
*Attention*, be aware that the latter version might not handle the case of a relative oid correctly. An
extra check might be necessary.

- To build an `Oid`, the `from`, `new` or `new_relative` methods can be used.
- The `from` method now returns a `Result` (failure can happen if the first components are too
  large, for ex)
- An `oid` macro has also been added in the `der-oid-macro` crate to easily build an `Oid` (see
  above).

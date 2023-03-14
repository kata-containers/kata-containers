<!-- cargo-sync-readme start -->

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE-MIT)
[![Apache License 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE-APACHE)
[![docs.rs](https://docs.rs/asn1-rs/badge.svg)](https://docs.rs/asn1-rs)
[![crates.io](https://img.shields.io/crates/v/asn1-rs.svg)](https://crates.io/crates/asn1-rs)
[![Download numbers](https://img.shields.io/crates/d/asn1-rs.svg)](https://crates.io/crates/asn1-rs)
[![Github CI](https://github.com/rusticata/asn1-rs/workflows/Continuous%20integration/badge.svg)](https://github.com/rusticata/asn1-rs/actions)
[![Minimum rustc version](https://img.shields.io/badge/rustc-1.53.0+-lightgray.svg)](#rust-version-requirements)

# BER/DER Parsers/Encoders

A set of parsers/encoders for Basic Encoding Rules (BER [[X.690]]) and Distinguished Encoding Rules(DER
[[X.690]]) formats, implemented with the [nom] parser combinator framework.

It is written in pure Rust, fast, and makes extensive use of zero-copy. A lot of care is taken
to ensure security and safety of this crate, including design (recursion limit, defensive
programming), tests, and fuzzing. It also aims to be panic-free.

This crate is a rewrite of [der-parser](https://crates.io/crates/der-parser) to propose a more data-oriented API,
and add generalized support for serialization.

Many ideas were borrowed from the [crypto/utils/der](https://github.com/RustCrypto/utils/tree/master/der) crate (like
the `Any`/`TryFrom`/`FromDer` mechanism), adapted and merged into a generalized BER/DER crate.
Credits (and many thanks) go to Tony Arcieri for writing the original crate.

# BER/DER parsers

BER stands for Basic Encoding Rules, and is defined in [[X.690]]. It defines a set of rules to
encode and decode ASN.1 [[X.680]] objects in binary.

[[X.690]] also defines Distinguished Encoding Rules (DER), which is BER with added rules to
ensure canonical and unequivocal binary representation of objects.

The choice of which one to use is usually guided by the speficication of the data format based
on BER or DER: for example, X.509 uses DER as encoding representation.

The main traits for parsing are the [`FromBer`] and [`FromDer`] traits.
These traits provide methods to parse binary input, and return either the remaining (unparsed) bytes
and the parsed object, or an error.

The parsers follow the interface from [nom], and the [`ParseResult`] object is a specialized version
of `nom::IResult`. This means that most `nom` combinators (`map`, `many0`, etc.) can be used in
combination to objects and methods from this crate. Reading the nom documentation may
help understanding how to write and combine parsers and use the output.

**Minimum Supported Rust Version**: 1.53.0

Note: if the `bits` feature is enabled, MSRV is 1.56.0 (due to `bitvec` 1.0)

# Recipes

See [doc::recipes] and [doc::derive] for more examples and recipes.

## Examples

Parse 2 BER integers:

```rust
use asn1_rs::{Integer, FromBer};

let bytes = [ 0x02, 0x03, 0x01, 0x00, 0x01,
              0x02, 0x03, 0x01, 0x00, 0x00,
];

let (rem, obj1) = Integer::from_ber(&bytes).expect("parsing failed");
let (rem, obj2) = Integer::from_ber(&bytes).expect("parsing failed");

assert_eq!(obj1, Integer::from_u32(65537));
```

In the above example, the generic [`Integer`] type is used. This type can contain integers of any
size, but do not provide a simple API to manipulate the numbers.

In most cases, the integer either has a limit, or is expected to fit into a primitive type.
To get a simple value, just use the `from_ber`/`from_der` methods on the primitive types:

```rust
use asn1_rs::FromBer;

let bytes = [ 0x02, 0x03, 0x01, 0x00, 0x01,
              0x02, 0x03, 0x01, 0x00, 0x00,
];

let (rem, obj1) = u32::from_ber(&bytes).expect("parsing failed");
let (rem, obj2) = u32::from_ber(&rem).expect("parsing failed");

assert_eq!(obj1, 65537);
assert_eq!(obj2, 65536);
```

If the parsing succeeds, but the integer cannot fit into the expected type, the method will return
an `IntegerTooLarge` error.

# BER/DER encoders

BER/DER encoding is symmetrical to decoding, using the traits `ToBer` and [`ToDer`] traits.
These traits provide methods to write encoded content to objects with the `io::Write` trait,
or return an allocated `Vec<u8>` with the encoded data.
If the serialization fails, an error is returned.

## Examples

Writing 2 BER integers:

```rust
use asn1_rs::{Integer, ToDer};

let mut writer = Vec::new();

let obj1 = Integer::from_u32(65537);
let obj2 = Integer::from_u32(65536);

let _ = obj1.write_der(&mut writer).expect("serialization failed");
let _ = obj2.write_der(&mut writer).expect("serialization failed");

let bytes = &[ 0x02, 0x03, 0x01, 0x00, 0x01,
               0x02, 0x03, 0x01, 0x00, 0x00,
];
assert_eq!(&writer, bytes);
```

Similarly to `FromBer`/`FromDer`, serialization methods are also implemented for primitive types:

```rust
use asn1_rs::ToDer;

let mut writer = Vec::new();

let _ = 65537.write_der(&mut writer).expect("serialization failed");
let _ = 65536.write_der(&mut writer).expect("serialization failed");

let bytes = &[ 0x02, 0x03, 0x01, 0x00, 0x01,
               0x02, 0x03, 0x01, 0x00, 0x00,
];
assert_eq!(&writer, bytes);
```

If the parsing succeeds, but the integer cannot fit into the expected type, the method will return
an `IntegerTooLarge` error.

## Changes

See `CHANGELOG.md`.

# References

- [[X.680]] Abstract Syntax Notation One (ASN.1): Specification of basic notation.
- [[X.690]] ASN.1 encoding rules: Specification of Basic Encoding Rules (BER), Canonical
  Encoding Rules (CER) and Distinguished Encoding Rules (DER).

[X.680]: http://www.itu.int/rec/T-REC-X.680/en "Abstract Syntax Notation One (ASN.1):
  Specification of basic notation."
[X.690]: https://www.itu.int/rec/T-REC-X.690/en "ASN.1 encoding rules: Specification of
  Basic Encoding Rules (BER), Canonical Encoding Rules (CER) and Distinguished Encoding Rules
  (DER)."
[nom]: https://github.com/Geal/nom "Nom parser combinator framework"
<!-- cargo-sync-readme end -->

## Changes

See `CHANGELOG.md`, and `UPGRADING.md` for instructions for upgrading major versions.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

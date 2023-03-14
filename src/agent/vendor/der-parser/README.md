<!-- cargo-sync-readme start -->

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE-MIT)
[![Apache License 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE-APACHE)
[![docs.rs](https://docs.rs/der-parser/badge.svg)](https://docs.rs/der-parser)
[![crates.io](https://img.shields.io/crates/v/der-parser.svg)](https://crates.io/crates/der-parser)
[![Download numbers](https://img.shields.io/crates/d/der-parser.svg)](https://crates.io/crates/der-parser)
[![dependency status](https://deps.rs/crate/der-parser/8.1.0/status.svg)](https://deps.rs/crate/der-parser/8.1.0)
[![Github CI](https://github.com/rusticata/der-parser/workflows/Continuous%20integration/badge.svg)](https://github.com/rusticata/der-parser/actions)
[![Minimum rustc version](https://img.shields.io/badge/rustc-1.53.0+-lightgray.svg)](#rust-version-requirements)

# BER/DER Parser

A parser for Basic Encoding Rules (BER [[X.690]]) and Distinguished Encoding Rules(DER
[[X.690]]), implemented with the [nom](https://github.com/Geal/nom) parser combinator
framework.

It is written in pure Rust, fast, and makes extensive use of zero-copy. A lot of care is taken
to ensure security and safety of this crate, including design (recursion limit, defensive
programming), tests, and fuzzing. It also aims to be panic-free.

Historically, this parser was intended for DER only, and BER support was added later. This may
still reflect on some naming schemes, but has no other consequence: the `BerObject` and
`DerObject` used in this crate are type aliases, so all functions are compatible.

DER parsing functions have additional constraints verification, however.

Serialization has also been added (see [Serialization](#serialization) )

The code is available on [Github](https://github.com/rusticata/der-parser)
and is part of the [Rusticata](https://github.com/rusticata) project.

# BER/DER parsers

BER stands for Basic Encoding Rules, and is defined in [X.690]. It defines a set of rules to
encode and decode ASN.1 objects in binary.

[X.690] also defines Distinguished Encoding Rules (DER), which is BER with added rules to
ensure canonical and unequivocal binary representation of objects.

The choice of which one to use is usually guided by the speficication of the data format based
on BER or DER: for example, X.509 uses DER as encoding representation.

See the related modules for object definitions, functions, and example:
- [`ber`]: Basic Encoding Rules
- [`der`]: Distinguished Encoding Rules

## Examples

Parse two BER integers (see [BER/DER Integers](#berder-integers)):

```rust
use der_parser::ber::parse_ber_integer;

let bytes = [ 0x02, 0x03, 0x01, 0x00, 0x01,
              0x02, 0x03, 0x01, 0x00, 0x00,
];

let (rem, obj1) = parse_ber_integer(&bytes).expect("parsing failed");
let (rem, obj2) = parse_ber_integer(&bytes).expect("parsing failed");
```

Parse a DER sequence of integers:

```rust
use der_parser::der::{parse_der_integer, parse_der_sequence_of};

let bytes = [ 0x30, 0x0a,
              0x02, 0x03, 0x01, 0x00, 0x01,
              0x02, 0x03, 0x01, 0x00, 0x00,
];

let (rem, seq) = parse_der_sequence_of(parse_der_integer)(&bytes)
                    .expect("parsing failed");
```

Note: all parsing functions return the remaining (unparsed) bytes and the parsed object, or an
error.

# DER parser design

Parsing functions are inspired from `nom`, and follow the same interface. The most common
return type is [`BerResult`](https://docs.rs/der-parser/latest/der_parser/error/type.BerResult.html), that stores the remaining bytes and
parsed [`BerObject`](https://docs.rs/der-parser/latest/der_parser/ber/struct.BerObject.html), or an error. Reading the nom documentation may
help understanding how to write parsers and use the output.

There are two different approaches for parsing DER objects: reading the objects recursively as
long as the tags are known, or specifying a description of the expected objects (generally from
the [ASN.1][X.680] description).

The first parsing method can be done using the [`parse_ber`](https://docs.rs/der-parser/latest/der_parser/ber/fn.parse_ber.html) and
[`parse_der`](https://docs.rs/der-parser/latest/der_parser/der/fn.parse_der.html) methods.
It is useful when decoding an arbitrary DER object.
However, it cannot fully parse all objects, especially those containing IMPLICIT, OPTIONAL, or
DEFINED BY items.

```rust
use der_parser::parse_der;

let bytes = [ 0x30, 0x0a,
              0x02, 0x03, 0x01, 0x00, 0x01,
              0x02, 0x03, 0x01, 0x00, 0x00,
];

let parsed = parse_der(&bytes);
```

The second (and preferred) parsing method is to specify the expected objects recursively. The
following functions can be used:
- [`parse_ber_sequence_defined`](https://docs.rs/der-parser/latest/der_parser/ber/fn.parse_ber_sequence_defined.html) and similar functions
for sequences and sets variants
- [`parse_ber_tagged_explicit`](https://docs.rs/der-parser/latest/der_parser/ber/fn.parse_ber_tagged_explicit.html) for tagged explicit
- [`parse_ber_tagged_implicit`](https://docs.rs/der-parser/latest/der_parser/ber/fn.parse_ber_tagged_implicit.html) for tagged implicit
- [`parse_ber_container`](https://docs.rs/der-parser/latest/der_parser/ber/fn.parse_ber_container.html) for generic parsing, etc.
- DER objects use the `_der_` variants

For example, to read a BER sequence containing two integers:

```rust
use der_parser::ber::*;
use der_parser::error::BerResult;

fn localparse_seq(i:&[u8]) -> BerResult {
    parse_ber_sequence_defined(|data| {
        let (rem, a) = parse_ber_integer(data)?;
        let (rem, b) = parse_ber_integer(rem)?;
        Ok((rem, vec![a, b]))
    })(i)
}

let bytes = [ 0x30, 0x0a,
              0x02, 0x03, 0x01, 0x00, 0x01,
              0x02, 0x03, 0x01, 0x00, 0x00,
];

let (_, parsed) = localparse_seq(&bytes).expect("parsing failed");

assert_eq!(parsed[0].as_u64(), Ok(65537));
assert_eq!(parsed[1].as_u64(), Ok(65536));
```

All functions return a [`BerResult`](https://docs.rs/der-parser/latest/der_parser/error/type.BerResult.html) object: the parsed
[`BerObject`](https://docs.rs/der-parser/latest/der_parser/ber/struct.BerObject.html), an `Incomplete` value, or an error.

Note that this type is also a `Result`, so usual functions (`map`, `unwrap` etc.) are available.

# Notes

## BER/DER Integers

DER integers can be of any size, so it is not possible to store them as simple integers (they
are stored as raw bytes).

Note that, by default, BER/DER integers are signed. Functions are provided to request reading
unsigned values, but they will fail if the integer value is negative.

To get the integer value for all possible integer sign and size, use
[`BerObject::as_bigint`](https://docs.rs/der-parser/latest/der_parser/ber/struct.BerObject.html#method.as_bigint)) (requires the `bigint` feature).

To get a simple value expected to be in a known range, use methods like
[`BerObject::as_i32`](ber/struct.BerObject.html#method.as_i32)) and
[`BerObject::as_i64`](ber/struct.BerObject.html#method.as_i64) (or the unsigned versions
[`BerObject::as_u32`](ber/struct.BerObject.html#method.as_u32) and
[`BerObject::as_u64`](ber/struct.BerObject.html#method.as_u64)
),
which will return the value, or an error if the integer is too large (or is negative).

```rust
use der_parser::ber::*;

let data = &[0x02, 0x03, 0x01, 0x00, 0x01];

let (_, object) = parse_ber_integer(data).expect("parsing failed");
assert_eq!(object.as_u64(), Ok(65537));

#[cfg(feature = "bigint")]
assert_eq!(object.as_bigint(), Ok(65537.into()))
```

Access to the raw value is possible using the `as_slice` method.

## Parsers, combinators, macros

Some parsing tools (for ex for tagged objects) are available in different forms:
- parsers: (regular) functions that takes input and create an object
- combinators: functions that takes parsers (or combinators) as input, and return a function
  (usually, the parser). They are used (combined) as building blocks to create more complex
  parsers.
- macros: these are generally previous (historic) versions of parsers, kept for compatibility.
  They can sometime reduce the amount of code to write, but are hard to debug.
  Parsers should be preferred when possible.

## Misc Notes

- The DER constraints are verified if using `parse_der`.
- `BerObject` and `DerObject` are the same objects (type alias). The only difference is the
  verification of constraints *during parsing*.

## Rust version requirements

The 7.0 series of `der-parser` requires **Rustc version 1.53 or greater**, based on `asn1-rs`
and `nom` 7 dependencies.

# Serialization

Support for encoding BER/DER objects is currently being tested and can be used by activating the `serialize` feature.
Note that current status is **experimental**.

See the `ber_encode_*` functions in the [`ber`](https://docs.rs/der-parser/latest/der_parser/ber/index.html) module, and
[`BerObject::to_vec`](https://docs.rs/der-parser/latest/der_parser/ber/struct.BerObject.html#method.to_vec)

# References

- [[X.680]] Abstract Syntax Notation One (ASN.1): Specification of basic notation.
- [[X.690]] ASN.1 encoding rules: Specification of Basic Encoding Rules (BER), Canonical
  Encoding Rules (CER) and Distinguished Encoding Rules (DER).

[X.680]: http://www.itu.int/rec/T-REC-X.680/en "Abstract Syntax Notation One (ASN.1):
  Specification of basic notation."
[X.690]: https://www.itu.int/rec/T-REC-X.690/en "ASN.1 encoding rules: Specification of
  Basic Encoding Rules (BER), Canonical Encoding Rules (CER) and Distinguished Encoding Rules
  (DER)."
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

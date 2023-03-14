[![Crates.io](https://img.shields.io/crates/v/picky-asn1-der.svg)](https://crates.io/crates/picky-asn1-der)
[![docs.rs](https://docs.rs/picky-asn1-der/badge.svg)](https://docs.rs/picky-asn1-der)
![Crates.io](https://img.shields.io/crates/l/picky-asn1-der)

Compatible with rustc 1.56.
Minimal rustc version bumps happen [only with minor number bumps in this project](https://github.com/Devolutions/picky-rs/issues/89#issuecomment-868303478).

# picky-asn1-der

Portions of project [serde_asn1_der](https://github.com/KizzyCode/serde_asn1_der) are held by
Keziah Biermann, 2019 as part of this project.

This crate implements an ASN.1-DER subset for serde.

The following types have built-in support:
 - `bool`: The ASN.1-BOOLEAN-type
 - `u8`, `u16`, `u32`, `u64`, `u128`, `usize`: The ASN.1-INTEGER-type
 - `()`: The ASN.1-NULL-type
 - `&[u8]`, `Vec<u8>`: The ASN.1-OctetString-type
 - `&str`, `String`: The ASN.1-UTF8String-type

More advanced types are supported through wrappers:
- Integer (as big integer)
- Bit String
- Object Identifier
- Utf8 String
- Numeric String
- Printable String
- IA5 String
- Generalized Time
- UTC Time
- Application Tags from 0 to 15
- Context Tags from 0 to 15

Everything sequence-like combined out of these types is also supported out of the box.

Check out doc.rs for tested code examples.

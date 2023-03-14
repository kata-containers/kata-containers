[![Crates.io](https://img.shields.io/crates/v/picky-asn1-x509.svg)](https://crates.io/crates/picky-asn1-x509)
[![docs.rs](https://docs.rs/picky-asn1-x509/badge.svg)](https://docs.rs/picky-asn1-x509)
![Crates.io](https://img.shields.io/crates/l/picky-asn1-x509)

Compatible with rustc 1.60.
Minimal rustc version bumps happen [only with minor number bumps in this project](https://github.com/Devolutions/picky-rs/issues/89#issuecomment-868303478).

# picky-asn1-x509

Provide implementation for types defined in [X.509 RFC](https://tools.ietf.org/html/rfc5280) and related RFC ([PKCS#8](https://tools.ietf.org/html/rfc5208), [PKCS#10](https://tools.ietf.org/html/rfc2986)).

This crate doesn't provide an easy to use API to create, read and validate X.509 certificates.
This is a low-level library providing only raw types for serialization and deserialization purposes.
These types are implementing `serde`'s `Serialize` and `Deserialize` and are to be used with [picky-asn1-der](https://crates.io/crates/picky-asn1-der).
If you're looking for a higher level API, you may be interested by the [picky crate](https://crates.io/crates/picky) which uses
this library internally and provides a nicer API.


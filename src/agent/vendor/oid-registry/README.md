<!-- cargo-sync-readme start -->

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE-MIT)
[![Apache License 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](./LICENSE-APACHE)
[![docs.rs](https://docs.rs/oid-registry/badge.svg)](https://docs.rs/oid-registry)
[![crates.io](https://img.shields.io/crates/v/oid-registry.svg)](https://crates.io/crates/oid-registry)
[![Github CI](https://github.com/rusticata/oid-registry/workflows/Continuous%20integration/badge.svg)](https://github.com/rusticata/oid-registry/actions)
[![Minimum rustc version](https://img.shields.io/badge/rustc-1.53.0+-lightgray.svg)](#rust-version-requirements)
# OID Registry

This crate is a helper crate, containing a database of OID objects. These objects are intended
for use when manipulating ASN.1 grammars and BER/DER encodings, for example.

This crate provides only a simple registry (similar to a `HashMap`) by default. This object can
be used to get names and descriptions from OID.

This crate provides default lists of known OIDs, that can be selected using the build features.
By default, the registry has no feature enabled, to avoid embedding a huge database in crates.

It also declares constants for most of these OIDs.

```rust
use oid_registry::OidRegistry;

let mut registry = OidRegistry::default()
    .with_crypto() // only if the 'crypto' feature is enabled
;

let e = registry.get(&oid_registry::OID_PKCS1_SHA256WITHRSA);
if let Some(entry) = e {
    // get sn: sha256WithRSAEncryption
    println!("sn: {}", entry.sn());
    // get description: SHA256 with RSA encryption
    println!("description: {}", entry.description());
}

```

## Extending the registry

These provided lists are often incomplete, or may lack some specific OIDs.
This is why the registry allows adding new entries after construction:

```rust
use asn1_rs::oid;
use oid_registry::{OidEntry, OidRegistry};

let mut registry = OidRegistry::default();

// entries can be added by creating an OidEntry object:
let entry = OidEntry::new("shortName", "description");
registry.insert(oid!(1.2.3.4), entry);

// when using static strings, a tuple can also be used directly for the entry:
registry.insert(oid!(1.2.3.5), ("shortName", "A description"));

```

## Versions and compatibility with `asn1-rs`

Versions of `oid-registry` must be chosen specifically, to depend on a precise version of `asn1-rs`.
The following table summarizes the matching versions:

- `oid-registry` 0.6.x depends on `asn1-rs` 0.5.0
- `oid-registry` 0.5.x depends on `asn1-rs` 0.4.0

## Contributing OIDs

All OID values, constants, and features are derived from files in the `assets` directory in the
build script (see `build.rs`).
See `load_file` for documentation of the file format.
<!-- cargo-sync-readme end -->

## Rust version requirements

`oid-registry` requires **Rustc version 1.53 or greater**, based on proc-macro
attributes support and `asn1-rs`.

# License

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

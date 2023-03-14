Continuous integration | Docs | License | Crate version | Crate downloads
 ----------------------|------|---------|---------------|-----------------
 [![Continuous integration](https://github.com/sigstore/sigstore-rs/actions/workflows/tests.yml/badge.svg)](https://github.com/sigstore/sigstore-rs/actions/workflows/tests.yml) | [![Docs](https://img.shields.io/badge/docs-%20-blue)](https://docs.rs/sigstore/latest/sigstore) |  [![License: Apache 2.0](https://img.shields.io/badge/License-Apache2.0-brightgreen.svg)](https://opensource.org/licenses/Apache-2.0) | [![Crate version](https://img.shields.io/crates/v/sigstore?style=flat-square)](https://crates.io/crates/sigstore) | [![Crate downloads](https://img.shields.io/crates/d/sigstore?style=flat-square)](https://crates.io/crates/sigstore)


This is an experimental crate to interact with [sigstore](https://sigstore.dev/).

This is under high development, many features and checks are still missing.

## Features

### Verification

The crate implements the following verification mechanisms:

  * Verify using a given key
  * Verify bundle produced by transparency log (Rekor)
  * Verify signature produced in keyless mode, using Fulcio Web-PKI

Signature annotations and certificate email can be provided at verification time.

### OpenID Connect

For use with Fulcio ephemeral key signing, an OpenID connect API is available.

#### Known limitations

* The crate does not handle verification of attestations yet.

## Examples

The `examples` directory contains demo programs using the library.

## Security

Should you discover any security issues, please refer to sigstores [security
process](https://github.com/sigstore/community/security/policy)

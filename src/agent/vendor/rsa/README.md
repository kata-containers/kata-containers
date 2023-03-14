# RSA

[![crates.io][crate-image]][crate-link]
[![Documentation][doc-image]][doc-link]
[![Build Status][build-image]][build-link]
![minimum rustc 1.56][msrv-image]
[![Project Chat][chat-image]][chat-link]
[![dependency status][deps-image]][deps-link]

A portable RSA implementation in pure Rust.

⚠️ **WARNING:** This crate has been audited by a 3rd party, but a full blog post with the results and the updates made since the audit has not been officially released yet. See [#60](https://github.com/RustCrypto/RSA/issues/60) for more information.

## Example

```rust
use rsa::{PublicKey, RsaPrivateKey, RsaPublicKey, PaddingScheme};

let mut rng = rand::thread_rng();
let bits = 2048;
let priv_key = RsaPrivateKey::new(&mut rng, bits).expect("failed to generate a key");
let pub_key = RsaPublicKey::from(&priv_key);

// Encrypt
let data = b"hello world";
let enc_data = pub_key.encrypt(&mut rng, PaddingScheme::new_pkcs1v15_encrypt(), &data[..]).expect("failed to encrypt");
assert_ne!(&data[..], &enc_data[..]);

// Decrypt
let dec_data = priv_key.decrypt(PaddingScheme::new_pkcs1v15_encrypt(), &enc_data).expect("failed to decrypt");
assert_eq!(&data[..], &dec_data[..]);
```

> **Note:** If you encounter unusually slow key generation time while using `RsaPrivateKey::new` you can try to compile in release mode or add the following to your `Cargo.toml`. Key generation is much faster when building with higher optimization levels, but this will increase the compile time a bit.
> ```toml
> [profile.debug]
> opt-level = 3
> ```
> If you don't want to turn on optimizations for all dependencies,
> you can only optimize the `num-bigint-dig` dependency. This should
> give most of the speedups.
> ```toml
> [profile.dev.package.num-bigint-dig]
> opt-level = 3
> ```

## Status

Currently at Phase 1 (v) 🚧

There will be three phases before `1.0` 🚢 can be released.

1. 🚧  Make it work
    - [x] Prime generation ✅
    - [x] Key generation ✅
    - [x] PKCS1v1.5: Encryption & Decryption ✅
    - [x] PKCS1v1.5: Sign & Verify ✅
    - [ ] PKCS1v1.5 (session key): Encryption & Decryption
    - [x] OAEP: Encryption & Decryption
    - [x] PSS: Sign & Verify
    - [x] Key import & export
2. 🚀 Make it fast
    - [x] Benchmarks ✅
    - [ ] compare to other implementations 🚧
    - [ ] optimize 🚧
3. 🔐 Make it secure
    - [ ] Fuzz testing
    - [ ] Security Audits


## Minimum Supported Rust Version (MSRV)

All crates in this repository support Rust 1.56 or higher. In future
minimally supported version of Rust can be changed, but it will be done with
a minor version bump.

## License

Licensed under either of

 * [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0)
 * [MIT license](http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[//]: # (badges)

[crate-image]: https://img.shields.io/crates/v/rsa.svg
[crate-link]: https://crates.io/crates/rsa
[doc-image]: https://docs.rs/rsa/badge.svg
[doc-link]: https://docs.rs/rsa
[build-image]: https://github.com/rustcrypto/RSA/workflows/CI/badge.svg
[build-link]: https://github.com/RustCrypto/RSA/actions?query=workflow%3ACI+branch%3Amaster
[msrv-image]: https://img.shields.io/badge/rustc-1.56+-blue.svg
[chat-image]: https://img.shields.io/badge/zulip-join_chat-blue.svg
[chat-link]: https://rustcrypto.zulipchat.com/#narrow/stream/260047-RSA
[deps-image]: https://deps.rs/repo/github/RustCrypto/RSA/status.svg
[deps-link]: https://deps.rs/repo/github/RustCrypto/RSA

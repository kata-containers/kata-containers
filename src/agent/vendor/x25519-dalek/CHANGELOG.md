# Changelog

Entries are listed in reverse chronological order.

## 1.1.1

* Fix a typo in the README.

## 1.1.0

* Add impls of `PartialEq`, `Eq`, and `Hash` for `PublicKey` (by @jack-michaud)

## 1.0.1

* Update underlying `curve25519_dalek` library to `3.0`.

## 1.0.0

* Widen generic bound on `EphemeralSecret::new` and `StaticSecret::new` to
  allow owned as well as borrowed RNGs.
* Add `PublicKey::to_bytes` and `SharedSecret::to_bytes`, returning owned byte
  arrays, complementing the existing `as_bytes` methods returning references.
* Remove mention of deprecated `rand_os` crate from examples.
* Clarify `EphemeralSecret`/`StaticSecret` distinction in documentation.

## 0.6.0

* Updates `rand_core` version to `0.5`.
* Adds `serde` support.
* Replaces `clear_on_drop` with `zeroize`.
* Use Rust 2018.

## 0.5.2

* Implement `Clone` for `StaticSecret`.

## 0.5.1

* Implement `Copy, Clone, Debug` for `PublicKey`.
* Remove doctests.

## 0.5.0

* Adds support for static and ephemeral keys.


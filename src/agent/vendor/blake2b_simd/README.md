# blake2b_simd [![GitHub](https://img.shields.io/github/tag/oconnor663/blake2_simd.svg?label=GitHub)](https://github.com/oconnor663/blake2_simd) [![crates.io](https://img.shields.io/crates/v/blake2b_simd.svg)](https://crates.io/crates/blake2b_simd) [![Build Status](https://travis-ci.org/oconnor663/blake2_simd.svg?branch=master)](https://travis-ci.org/oconnor663/blake2_simd)

An implementation of the BLAKE2b and BLAKE2bp hash functions. See also
[`blake2s_simd`](../blake2s).

This crate includes:

- 100% stable Rust.
- SIMD implementations based on Samuel Neves' [`blake2-avx2`](https://github.com/sneves/blake2-avx2).
  These are very fast. For benchmarks, see [the Performance section of the
  README](https://github.com/oconnor663/blake2_simd#performance).
- Portable, safe implementations for other platforms.
- Dynamic CPU feature detection. Binaries include multiple implementations by default and
  choose the fastest one the processor supports at runtime.
- All the features from the [the BLAKE2 spec](https://blake2.net/blake2.pdf), like adjustable
  length, keying, and associated data for tree hashing.
- `no_std` support. The `std` Cargo feature is on by default, for CPU feature detection and
  for implementing `std::io::Write`.
- Support for computing multiple BLAKE2b hashes in parallel, matching the efficiency of
  BLAKE2bp. See the [`many`](many/index.html) module.

# Example

```
use blake2b_simd::{blake2b, Params};

let expected = "ca002330e69d3e6b84a46a56a6533fd79d51d97a3bb7cad6c2ff43b354185d6d\
                c1e723fb3db4ae0737e120378424c714bb982d9dc5bbd7a0ab318240ddd18f8d";
let hash = blake2b(b"foo");
assert_eq!(expected, &hash.to_hex());

let hash = Params::new()
    .hash_length(16)
    .key(b"The Magic Words are Squeamish Ossifrage")
    .personal(b"L. P. Waterhouse")
    .to_state()
    .update(b"foo")
    .update(b"bar")
    .update(b"baz")
    .finalize();
assert_eq!("ee8ff4e9be887297cf79348dc35dab56", &hash.to_hex());
```

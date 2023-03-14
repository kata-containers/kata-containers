# sha1collisiondetection

Library and command line tool to detect SHA-1 collisions in files

This is a port of Marc Stevens' sha1collisiondetection algorithm to
Rust.  The code is translated from C to Rust using c2rust.

To improve the translation, the code is transformed first, replacing
macros with inline functions.  Running the test suite using `make
check` tests the correctness of the transformation.

This crate, like the C implementation, is distributed under the MIT
Software License.

# Installation of the command line utility

The command line utility is a drop-in replacement for coreutils'
`sha1sum` utility.  It can be installed, for example, using cargo:

```sh
$ cargo install sha1collisiondetection
[...]
$ sha1cdsum --help
sha1cdsum 0.2.3
Print or check SHA1 (160-bit) checksums with collision detection.

USAGE:
    sha1cdsum [FLAGS] [files]...
[...]
$ sha1cdsum test/*1.*
4f3d9be4a472c4dae83c6314aa6c36a064c1fd14 *coll* test/sha-mbles-1.bin
16e96b70000dd1e7c85b8368ee197754400e58ec *coll* test/shattered-1.pdf
```

# About

This library and command line tool were designed as near drop-in
replacements for common SHA-1 libraries and sha1sum.  They will
compute the SHA-1 hash of any given file and additionally will detect
cryptanalytic collision attacks against SHA-1 present in each file. It
is very fast and takes less than twice the amount of time as regular
SHA-1.

More specifically they will detect any cryptanalytic collision attack
against SHA-1 using any of the top 32 SHA-1 disturbance vectors with
probability 1:

```text
    I(43,0), I(44,0), I(45,0), I(46,0), I(47,0), I(48,0), I(49,0), I(50,0), I(51,0), I(52,0),
    I(46,2), I(47,2), I(48,2), I(49,2), I(50,2), I(51,2),
    II(45,0), II(46,0), II(47,0), II(48,0), II(49,0), II(50,0), II(51,0), II(52,0), II(53,0), II(54,0), II(55,0), II(56,0),
    II(46,2), II(49,2), II(50,2), II(51,2)
```

The possibility of false positives can be neglected as the probability
is smaller than 2^-90.

The library supports both an indicator flag that applications can
check and act on, as well as a special _safe-hash_ mode that returns
the real SHA-1 hash when no collision was detected and a different
_safe_ hash when a collision was detected.  Colliding files will have
the same SHA-1 hash, but will have different unpredictable
safe-hashes.  This essentially enables protection of applications
against SHA-1 collisions with no further changes in the application,
e.g., digital signature forgeries based on SHA-1 collisions
automatically become invalid.

For the theoretical explanation of collision detection see the
award-winning paper on _Counter-Cryptanalysis_:

Counter-cryptanalysis, Marc Stevens, CRYPTO 2013, Lecture Notes in
Computer Science, vol. 8042, Springer, 2013, pp. 129-146,
https://marc-stevens.nl/research/papers/C13-S.pdf

# Developers

The C implementation of the collision detection algorithm is
implemented by:

- Marc Stevens, CWI Amsterdam (https://marc-stevens.nl)
- Dan Shumow, Microsoft Research (https://www.microsoft.com/en-us/research/people/danshu/)

The C implementation is maintained
[here](https://github.com/cr-marcstevens/sha1collisiondetection).

Please report issues with the rust port
[here](https://gitlab.com/sequoia-pgp/sha1collisiondetection).

# Usage

```rust
use hex_literal::hex;
use sha1collisiondetection::{Sha1CD, Digest};

// create a Sha1CD object
let mut hasher = Sha1CD::new();

// process input message
hasher.update(b"hello world");

// acquire hash digest in the form of GenericArray,
// which in this case is equivalent to [u8; 20]
let result = hasher.finalize();
assert_eq!(result[..], hex!("2aae6c35c94fcfb415dbe95f408b9ce91ee846ed"));
```

# Feature flags

This crate uses *features* to enable or disable optional
functionality.  You can tweak the features in your `Cargo.toml` file,
like so:

```toml
sha1collisiondetection = { version = "*", default-features = false, features = [...] }
```

## Feature 'std'

If enabled, this crate requires the standard library.  Enabled by default.

## Feature 'digest-trait'

If enabled, this crate implements `trait Digest` from the `digest`
crate.  Enabled by default.

[![crates.io](https://img.shields.io/crates/v/nanorand.svg)](https://crates.io/crates/nanorand) [![docs.rs](https://docs.rs/nanorand/badge.svg)](https://docs.rs/nanorand)  [![License: Zlib](https://img.shields.io/badge/License-Zlib-brightgreen.svg)](https://opensource.org/licenses/Zlib) [![Tests](https://github.com/Absolucy/nanorand-rs/workflows/Tests/badge.svg?event=push&branch=master)](https://github.com/Absolucy/nanorand-rs/actions?query=workflow%3A%22Run+Tests%22) [![Average time to resolve an issue](https://isitmaintained.com/badge/resolution/Absolucy/nanorand-rs.svg)](https://isitmaintained.com/project/Absolucy/nanorand-rs "Average time to resolve an issue") [![Percentage of issues still open](https://isitmaintained.com/badge/open/Absolucy/nanorand-rs.svg)](https://isitmaintained.com/project/Absolucy/nanorand-rs "Percentage of issues still open") ![Maintenance](https://img.shields.io/badge/maintenance-activly--developed-brightgreen.svg)

# nanorand

Current version: **0.7.0**

A library meant for fast, random number generation with quick compile time, and minimal dependencies.

## Examples
### Generating a number with an initialized RNG
```rust
use nanorand::{Rng, WyRand};

let mut rng = WyRand::new();
println!("Random number: {}", rng.generate::<u64>());
```
### Generating a number with a thread-local RNG
```rust
use nanorand::Rng;

let mut rng = nanorand::tls_rng();
println!("Random number: {}", rng.generate::<u64>());
```
### Generating a number in a range
```rust
use nanorand::{Rng, WyRand};

let mut rng = WyRand::new();
println!("Random number between 1 and 100: {}", rng.generate_range(1_u64..=100));
println!("Random number between -100 and 50: {}", rng.generate_range(-100_i64..=50));
```
#### Buffering random bytes
```rust
use nanorand::{Rng, BufferedRng, WyRand};

let mut thingy = [0u8; 5];
let mut rng = BufferedRng::new(WyRand::new());
rng.fill(&mut thingy);
// As WyRand generates 8 bytes of output, and our target is only 5 bytes,
// 3 bytes will remain in the buffer.
assert_eq!(rng.buffered(), 3);
```
### Shuffling a Vec
```rust
use nanorand::{Rng, WyRand};

let mut rng = WyRand::new();
let mut items = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
rng.shuffle(&mut items);
```

### Why should I use this over...

* `rand` - The standard rand crate is a complex beast. It contains unsafe code in the core implementations, and while it has much more options than we do, that's kind of the point. We're straight to the point, while rand is everything and the kitchen sink.
* `fastrand`, `oorandom`, `random-fast-rng`, or `randomize` - These are all minimal, zero-dep implementations of the PCG family of RNGs (Pcg32 and Pcg64). While these are decent, they are _much_ slower than wyrand (which beats the speed of these Pcg32 implementations while providing 64 random bits), and do not provide CSPRNGs.
* `getrandom` - The getrandom crate just provides OS entropy sources. It is not meant for random number generation. In fact, we provide it as an optional entropy source.

### RNG Implementations

**RNG**|**nanorand type**|**Output Size**|**Cryptographically Secure**|**Speed**<sup>1</sup>|**Notes**|**Original Implementation**
:-----:|:-----:|:-----:|:-----:|:-----:|:-----:|:-----:
wyrand|[`nanorand::WyRand`](rand/wyrand/struct.WyRand.html), [`nanorand::tls::TlsWyRand`](tls/fn.tls_rng.html)|64 bits (`u64`)|≡ƒÜ½|16.4 GB/s||[https://github.com/lemire/testingRNG/blob/master/source/wyrand.h](https://github.com/lemire/testingRNG/blob/master/source/wyrand.h)
Pcg64|[`nanorand::Pcg64`](rand/pcg64/struct.Pcg64.html)|64 bits (`u64`)|≡ƒÜ½|1.6 GB/s||[https://github.com/rkern/pcg64](https://github.com/rkern/pcg64)
ChaCha|[`nanorand::ChaCha`](rand/chacha/struct.ChaCha.html)|512 bits (`[u32; 16]`)|Γ£à|204 MB/s (ChaCha8), 79 MB/s (ChaCha20)|Only works in Rust 1.47 or above|[https://cr.yp.to/chacha.html](https://cr.yp.to/chacha.html)

<sup>1. Speed benchmarked on an M1 Macbook Air</sup>

### Entropy Sources
_Listed in order of priority_

* If the `getrandom` feature is enabled, then [`getrandom::getrandom`](https://docs.rs/getrandom/*/getrandom/fn.getrandom.html) will be called.
* If the `rdseed` feature is enabled, and is running on an x86(-64) system with the [RDSEED](https://en.wikipedia.org/wiki/RDRAND) instruction, then
  we will attempt to source as much entropy as possible via our [`rdseed_entropy`](entropy::rdseed_entropy) function
* Linux and Android will attempt to use the [`getrandom`](https://man7.org/linux/man-pages/man2/getrandom.2.html) syscall.
* macOS and iOS (Darwin-based systems) will use Security.framework's [`SecRandomCopyBytes`](https://developer.apple.com/documentation/security/1399291-secrandomcopybytes).
* Windows
  * If we're targeting UWP, then the [`BCryptGenRandom`](https://docs.microsoft.com/en-us/windows/win32/api/bcrypt/nf-bcrypt-bcryptgenrandom) is used with system-preferred RNG (`BCRYPT_USE_SYSTEM_PREFERRED_RNG`).
  * Otherwise, we'll use [`RtlGenRandom`](https://docs.microsoft.com/en-us/windows/win32/api/ntsecapi/nf-ntsecapi-rtlgenrandom).

### Feature Flags

* `alloc` (default) - Enables Rust `alloc` lib features, such as a buffering Rng wrapper.
* `std` (default) - Enables Rust `std` lib features, such as seeding from OS entropy sources. Requires `alloc` to be enabled.
* `tls` (default) - Enables a thread-local [`WyRand`](rand/wyrand/struct.WyRand.html) RNG (see below). Requires `std` to be enabled.
* `wyrand` (default) - Enable the [`WyRand`](rand/wyrand/struct.WyRand.html) RNG.
* `pcg64` (default) - Enable the [`Pcg64`](rand/pcg64/struct.Pcg64.html)  RNG.
* `chacha` - Enable the [`ChaCha`](rand/chacha/struct.ChaCha.html) RNG. Requires Rust 1.47 or later.
* `rdseed` - On x86 and x86-64 platforms, the `rdseed` intrinsic will be used when OS entropy isn't available.
* `zeroize` - Implement the [Zeroize](https://crates.io/crates/zeroize) trait for all RNGs.
* `getrandom` - Use the [`getrandom`](https://crates.io/crates/getrandom) crate as an entropy source.
Works on most systems, optional due to the fact that it brings in more dependencies.

## License

The zlib/libpng License

Copyright (c) 2022 Lucy <lucy@absolucy.moe>

This software is provided 'as-is', without any express or implied warranty. In
no event will the authors be held liable for any damages arising from the use of
this software.

Permission is granted to anyone to use this software for any purpose, including
commercial applications, and to alter it and redistribute it freely, subject to
the following restrictions:

1.  The origin of this software must not be misrepresented; you must not claim
    that you wrote the original software. If you use this software in a product,
    an acknowledgment in the product documentation would be appreciated but is
    not required.

2.  Altered source versions must be plainly marked as such, and must not be
    misrepresented as being the original software.

3.  This notice may not be removed or altered from any source distribution.

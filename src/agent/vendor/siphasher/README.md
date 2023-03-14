SipHash implementation for Rust
===============================

This crates implements SipHash-2-4 and SipHash-1-3 in Rust.

It is based on the original implementation from rust-core and exposes the
same API.

It also implements SipHash variants returning 128-bit tags.

The `sip` module implements the standard 64-bit mode, whereas the `sip128`
module implements the 128-bit mode.

Usage
-----
In `Cargo.toml`:

```toml
[dependencies]
siphasher = "0.3"
```

If you want [serde](https://github.com/serde-rs/serde) support, include the feature like this:

```toml
[dependencies]
siphasher = { version = "0.3", features = ["serde"] }
```

64-bit mode:
```rust
extern crate siphasher;

use siphasher::sip::{SipHasher, SipHasher13, SipHasher24};
```

128-bit mode:
```rust
use siphasher::sip128::{Hasher128, Siphasher, SipHasher13, SipHasher24};
```

[API documentation](https://docs.rs/siphasher/)
-----------------------------------------------

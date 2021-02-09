# Slab

Pre-allocated storage for a uniform data type.

[![Crates.io](https://img.shields.io/crates/v/slab.svg?maxAge=2592000)](https://crates.io/crates/slab)
[![Build Status](https://travis-ci.org/carllerche/slab.svg?branch=master)](https://travis-ci.org/carllerche/slab)

[Documentation](https://docs.rs/slab/0.4.2/slab/)

## Usage

To use `slab`, first add this to your `Cargo.toml`:

```toml
[dependencies]
slab = "0.4.2"
```

Next, add this to your crate:

```rust
extern crate slab;

use slab::Slab;

let mut slab = Slab::new();

let hello = slab.insert("hello");
let world = slab.insert("world");

assert_eq!(slab[hello], "hello");
assert_eq!(slab[world], "world");

slab[world] = "earth";
assert_eq!(slab[world], "earth");
```

See [documentation](https://docs.rs/slab) for more details.

## License

This project is licensed under the [MIT license](LICENSE).

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in `slab` by you, shall be licensed as MIT, without any additional
terms or conditions.

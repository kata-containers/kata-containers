# rlimit

[![Latest Version]][crates.io]
[![Documentation]][docs.rs] 
![License]

A simple wrapper for `getrlimit` and `setrlimit`.

[crates.io]: https://crates.io/crates/rlimit
[Latest Version]: https://img.shields.io/crates/v/rlimit.svg
[Documentation]: https://docs.rs/rlimit/badge.svg
[docs.rs]: https://docs.rs/rlimit
[License]: https://img.shields.io/crates/l/rlimit.svg

## Examples

### Set resource limit

```rust
use rlimit::{setrlimit, Resource, Rlim};

const DEFAULT_SOFT_LIMIT: Rlim = Rlim::from_raw(4 * 1024 * 1024);
const DEFAULT_HARD_LIMIT: Rlim = Rlim::from_raw(8 * 1024 * 1024);
assert!(Resource::FSIZE.set(DEFAULT_SOFT_LIMIT, DEFAULT_HARD_LIMIT).is_ok());

let soft = Rlim::from_usize(16384);
let hard = soft * 2;
assert!(setrlimit(Resource::NOFILE, soft, hard).is_ok());
```

### Get resource limit

```rust
use rlimit::{getrlimit, Resource, Rlim};

assert!(Resource::NOFILE.get().is_ok());
assert_eq!(getrlimit(Resource::CPU).unwrap(), (Rlim::INFINITY, Rlim::INFINITY));
```

### Increase NOFILE limit

See the example [nofile](https://github.com/Nugine/rlimit/tree/v0.5.4/examples/nofile.rs).

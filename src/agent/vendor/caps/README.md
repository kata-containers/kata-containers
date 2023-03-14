# caps

[![Build Status](https://travis-ci.com/lucab/caps-rs.svg?branch=master)](https://travis-ci.com/lucab/caps-rs)
[![crates.io](https://img.shields.io/crates/v/caps.svg)](https://crates.io/crates/caps)
[![Documentation](https://docs.rs/caps/badge.svg)](https://docs.rs/caps)

A pure-Rust library to work with Linux capabilities.

`caps` provides support for manipulating capabilities available in modern Linux
kernels. It supports traditional POSIX sets (Effective, Inheritable, Permitted)
as well as Linux-specific Ambient and Bounding capabilities sets.

`caps` provides a simple and idiomatic interface to handle capabilities on Linux.
See `capabilities(7)` for more details.

## Motivations

This library tries to achieve the following goals:
 * fully support modern kernels, including recent capabilities and sets
 * provide an idiomatic interface
 * be usable in static targets, without requiring an external C library

## Example

```rust
type ExResult<T> = Result<T, Box<dyn std::error::Error + 'static>>;

fn manipulate_caps() -> ExResult<()> {
    use caps::{Capability, CapSet};

    // Retrieve permitted set.
    let cur = caps::read(None, CapSet::Permitted)?;
    println!("Current permitted caps: {:?}.", cur);
    
    // Retrieve effective set.
    let cur = caps::read(None, CapSet::Effective)?;
    println!("Current effective caps: {:?}.", cur);
    
    // Check if CAP_CHOWN is in permitted set.
    let perm_chown = caps::has_cap(None, CapSet::Permitted, Capability::CAP_CHOWN)?;
    if !perm_chown {
        return Err("Try running this as root!".into());
    }

    // Clear all effective caps.
    caps::clear(None, CapSet::Effective)?;
    println!("Cleared effective caps.");
    let cur = caps::read(None, CapSet::Effective)?;
    println!("Current effective caps: {:?}.", cur);

    // Since `CAP_CHOWN` is still in permitted, it can be raised again.
    caps::raise(None, CapSet::Effective, Capability::CAP_CHOWN)?;
    println!("Raised CAP_CHOWN in effective set.");
    let cur = caps::read(None, CapSet::Effective)?;
    println!("Current effective caps: {:?}.", cur);

    Ok(())
}
```

Some more examples are available under [examples](examples).

## License

Licensed under either of

 * MIT license - <http://opensource.org/licenses/MIT>
 * Apache License, Version 2.0 - <http://www.apache.org/licenses/LICENSE-2.0>

at your option.

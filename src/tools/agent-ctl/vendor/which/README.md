[![Build Status](https://github.com/harryfei/which-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/harryfei/which-rs/actions/workflows/rust.yml)

# which

A Rust equivalent of Unix command "which". Locate installed executable in cross platforms.

## Support platforms

* Linux
* Windows
* macOS

## Example

To find which rustc exectable binary is using.

``` rust
use which::which;

let result = which::which("rustc").unwrap();
assert_eq!(result, PathBuf::from("/usr/bin/rustc"));
```

## Documentation

The documentation is [available online](https://docs.rs/which/).

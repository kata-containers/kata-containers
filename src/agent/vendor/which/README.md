[![Travis Build Status](https://travis-ci.org/harryfei/which-rs.svg?branch=master)](https://travis-ci.org/harryfei/which-rs)
[![Appveyor Build status](https://ci.appveyor.com/api/projects/status/1y40b135iaixs9x6?svg=true)](https://ci.appveyor.com/project/HarryFei/which-rs)

# which

A Rust equivalent of Unix command "which". Locate installed execuable in cross platforms.

## Support platforms

* Linux
* Windows
* macOS

## Example

To find which rustc exectable binary is using. Locate installed execuable in cross platforms.

``` rust
use which::which;

let result = which::which("rustc").unwrap();
assert_eq!(result, PathBuf::from("/usr/bin/rustc"));

```

## Documentation

The documentation is [available online](https://docs.rs/which/).

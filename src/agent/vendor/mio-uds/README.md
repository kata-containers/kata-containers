# mio-uds

A library for integrating Unix Domain Sockets with [mio]. Based on the standard
library's [support for Unix sockets][std], except all of the abstractions and
types are nonblocking to conform with the expectations of mio.

[mio]: https://github.com/tokio-rs/mio
[std]: https://doc.rust-lang.org/std/os/unix/net/

[![Build Status](https://github.com/deprecrated/mio-uds/workflows/CI/badge.svg)](https://github.com/deprecrated/mio-uds/actions?query=workflow%3ACI+branch%3Amaster)
[![Documentation](https://docs.rs/mio-uds/badge.svg?version=0.6)](https://docs.rs/mio-uds/~0.6)

# mio-uds is Deprecated

With the 0.7 release, `mio` now includes native support for Unix Domain
Sockets.  Consumers should switch to that functionality when updating their
`mio` dependency to the 0.7.x series.


## Usage

```toml
# Cargo.toml
[dependencies]
mio-uds = "0.6"
mio = "0.6"
```

The three exported types at the top level, `UnixStream`, `UnixListener`, and
`UnixDatagram`, are thin wrappers around the libstd counterparts. They can be
used in similar fashions to mio's TCP and UDP types in terms of registration and
API.

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

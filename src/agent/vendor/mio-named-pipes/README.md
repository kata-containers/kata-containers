# mio-named-pipes

![Run tests](https://github.com/bbqsrc/mio-named-pipes/workflows/Run%20tests/badge.svg)

[Documentation](https://docs.rs/mio-named-pipes/0.1/x86_64-pc-windows-msvc/mio_named_pipes/)

A library for integrating Windows [Named Pipes] with [mio].

[Named Pipes]: https://msdn.microsoft.com/en-us/library/windows/desktop/aa365590(v=vs.85).aspx
[mio]: https://github.com/carllerche/mio

```toml
# Cargo.toml
[dependencies]
mio-named-pipes = "0.1"
mio = "0.6"
```

## Usage

The primary type, `NamedPipe`, can be constructed with `NamedPipe::new` or
through the `IntoRawHandle` type. All operations on `NamedPipe` are nonblocking
and will return an I/O error if they'd block (with the error indicating so).

Typically you can use a `NamedPipe` in the same way you would a TCP socket on
Windows with mio.

> **Note**: Named pipes on Windows do not have a zero-cost abstraction when
> working with the mio interface (readiness, not completion). As a result, this
> library internally has some buffer management that hasn't been optimized yet.
> It's recommended you benchmark this library for your application, and feel
> free to contact me if anything looks awry.

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in mio-named-pipes by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

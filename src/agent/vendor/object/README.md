# `object`

The `object` crate provides a unified interface to working with object files
across platforms. It supports reading object files and executable files,
and writing object files.

For reading files, it provides multiple levels of support:

* raw struct definitions suitable for zero copy access
* low level APIs for accessing the raw structs
* a higher level unified API for accessing common features of object files, such
  as sections and symbols

Supported file formats: ELF, Mach-O, Windows PE/COFF, and Unix archive.

## License

Licensed under either of

  * Apache License, Version 2.0 ([`LICENSE-APACHE`](./LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
  * MIT license ([`LICENSE-MIT`](./LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

See [CONTRIBUTING.md](./CONTRIBUTING.md) for hacking.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

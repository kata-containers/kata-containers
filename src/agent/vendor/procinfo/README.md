# procinfo

[![Build Status](https://travis-ci.org/danburkert/procinfo-rs.svg?branch=master)](https://travis-ci.org/danburkert/procinfo-rs)

[Documentation](https://docs.rs/procinfo)

A Rust library for reading information from `/proc`, the Linux process
information psuedo-filesystem. `procinfo` provides a simple interface for inspecting
process and system information on Linux.

## Status

The goal is that `procinfo` will provide interfaces for all of the files in `/proc`,
currently the following interfaces are provided:

* `/proc/loadavg`
* `/proc/<pid>/cwd`
* `/proc/<pid>/limits`
* `/proc/<pid>/mountinfo`
* `/proc/<pid>/stat`
* `/proc/<pid>/statm`
* `/proc/<pid>/status`
* `/proc/sys/fs/file-max`
* `/proc/net/dev`

`procinfo` requires Rust 1.13 or later.

## Contributing

Contributions will be gladly accepted for new `/proc` file parsers.  In addition
to parsers, help is needed testing `procinfo` on uncommon, old, bleeding edge,
containerized, and namespaced kernels. If you find that any of the documentation
is misleading, incomplete, or insufficient, please file an issue!

## License

`procinfo` is primarily distributed under the terms of both the MIT license and the
Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT) for details.

Copyright (c) 2015 Dan Burkert.

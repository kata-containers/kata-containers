# petgraph

Graph data structure library. Please read the [API documentation here][].

Supports Rust 1.41 and later (some older versions may require picking the dependency versions [by hand][dependency_hack]).

[![build_status][]](https://github.com/petgraph/petgraph/actions) [![crates][]](https://crates.io/crates/petgraph) [![gitter][]](https://gitter.im/petgraph-rs/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

Crate feature flags:

-   `graphmap` (default) enable `GraphMap`.
-   `stable_graph` (default) enable `StableGraph`.
-   `matrix_graph` (default) enable `MatrixGraph`.
-   `serde-1` (optional) enable serialization for `Graph, StableGraph`
    using serde 1.0. Requires Rust version as required by serde.

## Recent Changes

See [RELEASES][] for a list of changes. The minimum supported rust
version will only change on major releases.

## License

Dual-licensed to be compatible with the Rust project.

Licensed under the Apache License, Version 2.0
<http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
<http://opensource.org/licenses/MIT>, at your option. This file may not
be copied, modified, or distributed except according to those terms.

  [API documentation here]: https://docs.rs/petgraph/
  [build_status]: https://github.com/petgraph/petgraph/workflows/Continuous%20integration/badge.svg?branch=master
  [crates]: https://img.shields.io/crates/v/petgraph
  [gitter]: https://badges.gitter.im/petgraph-rs/community.svg
  [RELEASES]: RELEASES.rst
  [dependency_hack]: https://github.com/petgraph/petgraph/pull/493#issuecomment-1134970689

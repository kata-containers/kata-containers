# Testing

The `virtio-queue` crate is tested using:
- unit tests - defined in their corresponding modules,
- performance tests - defined in the [benches](../benches) directory. For now,
  the benchmarks are not run as part of the CI, but they can be run locally.

The crate provides a mocking framework for the driver side of a virtio queue,
in the [mock](../src/mock.rs) module.
This module is compiled only when using the `test-utils` feature. To run all
the unit tests (which include the documentation examples), and the performance
tests in this crate, you need to specify the `test-utils` feature, otherwise
the build fails.

```bash
cargo test --features test-utils
cargo bench --features test-utils
cargo test --doc --features test-utils
```

The mocking framework and the helpers it provides can be used in other crates
as well in order to test, for example, a specific device implementation. To be
able to use these test utilities, add the following to your `Cargo.toml` in the
`[dev-dependencies]` section:

```toml
[dev-dependencies]
virtio-queue = { version = "0.1.0", features = ["test-utils"] }
```

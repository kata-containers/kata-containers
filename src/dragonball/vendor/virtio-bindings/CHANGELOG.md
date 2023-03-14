# v0.1.0

This is the first `virtio-bindings` crate release.

This crate provides Rust FFI bindings to the
[Virtual I/O Device (VIRTIO)](https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.html)
Linux kernel API. With this first release, the bindings are for the Linux kernel
versions 4.14 and 5.0.

The bindings are generated using [bindgen](https://crates.io/crates/bindgen).

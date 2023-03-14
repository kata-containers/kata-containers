# virtio-bindings
Rust FFI bindings to virtio generated using [bindgen](https://crates.io/crates/bindgen).

# Usage
Add this to your `Cargo.toml`:
```toml
virtio-bindings = { version = "0.1", features = ["virtio-v5_0_0"]}
```
You can then import the bindings where you need them. As an example, to grab the
bindings for virtio-blk, you can do:
```rust
use virtio_bindings::bindings::virtio_blk::*;
```
In the `virtio-bindings` crate each feature maps to exactly one Linux version as follows:
- `virtio-v4_14_0` contains the bindings for the Linux kernel version 4.14
- `virtio-v5_0_0` contains the bindings for the Linux kernel version 5.0

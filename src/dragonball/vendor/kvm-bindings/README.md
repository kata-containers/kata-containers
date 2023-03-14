[![Build Status](https://travis-ci.org/rust-vmm/kvm-bindings.svg?branch=master)](https://travis-ci.org/rust-vmm/kvm-bindings)
[![Crates.io](https://img.shields.io/crates/v/kvm-bindings.svg)](https://crates.io/crates/kvm-bindings)
![](https://img.shields.io/crates/l/kvm-bindings.svg)
# kvm-bindings
Rust FFI bindings to KVM, generated using
[bindgen](https://crates.io/crates/bindgen). It currently has support for the
following target architectures:
- x86
- x86_64
- arm
- arm64

The bindings exported by this crate are statically generated using header files
associated with a specific kernel version, and are not automatically synced with
the kernel version running on a particular host. The user must ensure that
specific structures, members, or constants are supported and valid for the
kernel version they are using. For example, the `immediate_exit` field from the
`kvm_run` structure is only meaningful if the `KVM_CAP_IMMEDIATE_EXIT`
capability is available. Using invalid fields or features may lead to undefined
behaviour.

# Usage
First, add the following to your `Cargo.toml`:
```toml
kvm-bindings = "0.3"
```
Next, add this to your crate root:
```rust
extern crate kvm_bindings;
```

This crate also offers safe wrappers over FAM structs - FFI structs that have
a Flexible Array Member in their definition.
These safe wrappers can be used if the `fam-wrappers` feature is enabled for
this crate. Example:
```toml
kvm-bindings = { version = "0.3", features = ["fam-wrappers"]}
```

# Dependencies
The crate has an `optional` dependency to
[vmm-sys-util](https://crates.io/crates/vmm-sys-util) when enabling the
`fam-wrappers` feature.

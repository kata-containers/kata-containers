# Contributing to virtio-bindings

## Dependencies

### Bindgen
The bindings are currently generated using
[bindgen](https://crates.io/crates/bindgen) version 0.49.0:
```bash
cargo install bindgen --vers 0.49.0
```

### Linux Kernel
Generating bindings depends on the Linux kernel, so you need to have the
repository on your machine:

```bash
git clone https://github.com/torvalds/linux.git
```

## Example for adding a new version

For this example we assume that you have both linux and virtio-bindings
repositories in your root.

```bash
# Step 1: Crate a new module using a name with format "bindings_vVERSION" in
# src/
cd virtio-bindings
mkdir src/bindings_v5_0_0
cd ~

# Step 2: Copy the "mod.rs" file from the directory of an already existing
# version module to the one we've just created.
cd virtio-bindings/src
cp bindings_v4_14_0/mod.rs bindings_v5_0_0/mod.rs

# linux is the repository that you cloned at the previous step.
cd linux
# Step 3: Checkout the version you want to generate the bindings for.
git checkout v5.0

# Step 4: Generate the bindings from the kernel headers. We need to
# generate a file for each one of the virtio headers we're interested on.
# For the moment, we're generating "virtio_blk", "virtio_net" and
# "virtio_ring". Feel free to add additional header files if you need them
# for your project.
make headers_install INSTALL_HDR_PATH=v5_0_headers
cd v5_0_headers
for i in virtio_blk virtio_net virtio_ring ; do \
    bindgen include/linux/$i.h -o $i.rs \
    --with-derive-default \
    --with-derive-partialeq \
    -- -Iinclude
done
cd ~

# Step 6: Copy the generated files to the new version module.
cp linux/v5_0_headers/*.rs virtio-bindings/src/bindings_v5_0_0
```

Once this is done, edit the generated files to add the proper license header,
and add the new version module to `virtio-bindings/lib.rs`. If this version
is newer than the others already present, make this version the default one
by getting it imported when there isn't any other version specified as a
feature:

```rust
#[cfg(all(not(feature = "virtio-v4_14_0"), not(feature = "virtio-v5_0_0")))]
pub use super::bindings_v5_0_0::*;
```

# vsock-rs

Virtio socket support for Rust. Implements VsockListener and VsockStream
which are analogous to the `std::net::TcpListener` and `std::net::TcpStream` types. 

## Usage

Refer to the crate [documentation](https://docs.rs/vsock).

## Testing

### Prerequisites

You will need a recent qemu-system-x86_64 build in your path.

### Host

Setup the required virtio kernel modules:

```
make kmod
```

Start the test vm, you can shutdown the vm with the keyboard shortcut ```Ctrl+A``` and then ```x```:

```
make vm
```

### Tests

Run the test suite with:

```
make check
```
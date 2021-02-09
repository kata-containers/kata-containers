# tokio-vsock

Asynchronous Virtio socket support for Rust. The implementation is 
based off of Tokio and Mio's `TCPListener` and `TCPStream` interfaces.

tokio-vsock is for the most part **pre-alpha** quality, so there are probably 
**sharp edges**. Please test it thoroughly before using in production. Happy to receive
pull requests and issue reports.

## Use Cases

The most common use case for tokio-vsock would be writing agents for microvm
applications. Examples would include container runtimes.

## Usage

Refer to the crate [documentation](https://docs.rs/tokio-vsock/).

## Testing

### Prerequisites

You will need a recent qemu-system-x86_64 build in your path.

### Host

Setup the required Virtio kernel modules:

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

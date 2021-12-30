# shim-proto

Protobuf message and ttRPC service definitions for Containerd shim v2 protocol.

## Design

The `shim-proto` crate provides [Protobuf](https://github.com/protocolbuffers/protobuf.git) message
and [ttRPC](https://github.com/containerd/ttrpc.git) service definitions for the
[Containerd shim v2](https://github.com/containerd/containerd/blob/main/runtime/v2/task/shim.proto) protocol.
Details about `shim-v2` can be found [here](https://github.com/kata-containers/kata-containers/tree/main/docs/design/architecture#shim-v2-architecture)

The message and service definition source files are auto-generated from the
[shim-v2 protobuf source file](https://github.com/containerd/containerd/blob/main/runtime/v2/task/shim.proto)
by using [ttrpc-codegen](https://github.com/containerd/ttrpc-rust/tree/master/ttrpc-codegen). So please do not
edit those auto-generated source files. If upgrading/modifications are needed, please follow the steps:
- Synchronize the latest protobuf files form original sources into directory 'proto/'.
- Re-generate the source files by `cargo build --features=generate`.
- Commit the synchornized protobuf files and auto-generated source files, keeping them in synchronization.


## Usage

Add `shim-proto` as a dependency in your `Cargo.toml`

```toml
[dependencies]
shim-proto = "0.1"
```

Then add `extern crate shim-proto;` to your crate root.

## Examples
- [a server to provide shim-v2 service](./examples/shim-proto-ttrpc-server.rs)
- [a client to access shim-v2 service](./examples/shim-proto-ttrpc-client.rs)

## License

This project is licensed under

- [Apache License](http://www.apache.org/licenses/LICENSE-2.0), Version 2.0

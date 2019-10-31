# Kata Agent in Rust

This is a rust version of the [`kata-agent`](https://github.com/kata-containers/kata-agent).

In Denver PTG, [we discussed about re-writing agent in rust](https://etherpad.openstack.org/p/katacontainers-2019-ptg-denver-agenda):

> In general, we all think about re-write agent in rust to reduce the footprint of agent. Moreover, Eric mentioned the possibility to stop using gRPC, which may have some impact on footprint. We may begin to do some PoC to show how much we could save by re-writing agent in rust.

After that, we drafted the initial code here, and any contributions are welcome.

## Features

| Feature | Status |
| :--|:--:|
| **OCI Behaviors** |
| create/start containers | :white_check_mark: |
| signal/wait process     | :white_check_mark: |
| exec/list process       | :white_check_mark: |
| I/O stream              | :white_check_mark: |
| Cgroups                 | :white_check_mark: |
| Capabilities, rlimit, readonly path, masked path, users | :white_check_mark: |
| container stats (`stats_container`)                     | :white_check_mark: |
| Hooks                   | :white_check_mark: |
| **Agent Features & APIs** |
| run agent as `init` (mount fs, udev, setup `lo`) | :white_check_mark: |
| block device as root device                      | :white_check_mark: |
| Health API                                       | :white_check_mark: |
| network, interface/routes (`update_container`)   | :white_check_mark: |
| File transfer API (`copy_file`)                  | :white_check_mark: |
| Device APIs (`reseed_random_device`, , `online_cpu_memory`, `mem_hotplug_probe`, `set_guet_data_time`) | :white_check_mark: |
| vsock support                                    | :white_check_mark: |
| virtio-serial support                            | :heavy_multiplication_x: |
| OCI Spec validator                               | :white_check_mark: |
| **Infrastructures**|
| Debug Console | :white_check_mark: |
| Command line  | :white_check_mark: |
| Tracing       | :heavy_multiplication_x: |

## Getting Started

### Dependencies
The `rust-agent` depends on [`grpc-rs`](https://github.com/pingcap/grpc-rs) by PingCAP. However, the upstream `grpc-rs` and [gRPC](https://github.com/grpc/grpc) need some changes to be used here, which may take some time to be landed. Therefore, we created a temporary fork or `grpc-rs` here:
- https://github.com/alipay/grpc-rs/tree/rust_agent

### Build from Source
The rust-agent need to be built with rust nightly, and static linked with musl.
```bash
rustup toolchain install nightly
rustup default nightly
rustup target add x86_64-unknown-linux-musl --toolchain=nightly
git submodule update --init --recursive  
sudo ln -s /usr/bin/g++ /bin/musl-g++  
cargo build --target x86_64-unknown-linux-musl --release
```

## Run Kata CI with rust-agent
   * Firstly, install kata as noted by ["how to install Kata"](https://github.com/kata-containers/documentation/blob/master/install/README.md)
   * Secondly, build your own kata initrd/image following the steps in ["how to build your own initrd/image"](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#create-and-install-rootfs-and-initrd-image).
notes: Please use your rust agent instead of the go agent when building your initrd/image.
   * Clone the kata ci test cases from: https://github.com/kata-containers/tests.git, and then run the cri test with: 

```bash
$sudo -E PATH=$PATH -E GOPATH=$GOPATH integration/containerd/shimv2/shimv2-tests.sh
```

## Mini Benchmark
The memory of 'RssAnon' consumed by the go-agent and rust-agent as below:
go-agent: about 11M
rust-agent: about 1.1M

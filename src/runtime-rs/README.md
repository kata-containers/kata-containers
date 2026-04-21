# runtime-rs

## What is runtime-rs

`runtime-rs` is a core component of Kata Containers 4.0. It is a high-performance, Rust-based implementation of the containerd shim v2 runtime.

Key characteristics:

- **Implementation Language**: Rust, leveraging memory safety and zero-cost abstractions
- **Project Maturity**: Production-ready component of Kata Containers 4.0
- **Architectural Design**: Modular framework optimized for Kata Containers 4.0

For architecture details, see [Architecture Overview](../../docs/design/architecture_4.0).

## Architecture Overview

Key features:

- **Built-in VMM (Dragonball)**: Deeply integrated into shim lifecycle, eliminating IPC overhead for peak performance
- **Asynchronous I/O**: Tokio-based async runtime for high-concurrency with reduced thread footprint
- **Extensible Framework**: Pluggable hypervisors, network interfaces, and storage backends
- **Resource Lifecycle Management**: Comprehensive sandbox and container resource management

![crates overview](docs/images/crate-overview.svg)

## Crates

| Crate | Description |
|-------|-------------|
| [`shim`](crates/shim) | Containerd shim v2 entry point (start, delete, run commands) |
| [`service`](crates/service) | Services including TaskService for containerd shim protocol |
| [`runtimes`](crates/runtimes) | Runtime handlers: VirtContainer (default), LinuxContainer(experimental), WasmContainer(experimental) |
| [`resource`](crates/resource) | Resource management: network, share_fs, rootfs, volume, cgroups, cpu_mem |
| [`hypervisor`](crates/hypervisor) | Hypervisor implementations |
| [`agent`](crates/agent) | Guest agent communication (KataAgent) |
| [`persist`](crates/persist) | State persistence to disk (JSON format) |
| [`shim-ctl`](crates/shim-ctl) | Development tool for testing shim without containerd |

### shim

Entry point implementing [containerd shim v2 binary protocol](https://github.com/containerd/containerd/tree/main/runtime/v2#commands):

- `start`: Start new shim process
- `delete`: Delete existing shim process
- `run`: Run ttRPC service

### service

Extensible service framework. Currently implements `TaskService` conforming to [containerd shim protocol](https://docs.rs/containerd-shim-protos/).

### runtimes

Runtime handlers manage sandbox and container operations:

| Handler | Feature Flag | Description |
|---------|--------------|-------------|
| `VirtContainer` | `virt` (default) | Virtual machine-based containers |
| `LinuxContainer` | `linux` | Linux container runtime (experimental) |
| `WasmContainer` | `wasm` | WebAssembly runtime (experimental) |

### resource

All resources abstracted uniformly:

- **Sandbox resources**: network, share-fs
- **Container resources**: rootfs, volume, cgroup

Sub-modules: `cpu_mem`, `cdi_devices`, `coco_data`, `network`, `share_fs`, `rootfs`, `volume`

### hypervisor

Supported hypervisors:

| Hypervisor | Mode | Description |
|------------|------|-------------|
| Dragonball | Built-in | Integrated VMM for peak performance (default) |
| QEMU | External | Full-featured emulator |
| Cloud Hypervisor | External | Modern VMM (x86_64, aarch64) |
| Firecracker | External | Lightweight microVM |
| Remote | External | Remote hypervisor |

The built-in VMM mode (Dragonball) is recommended for production, offering superior performance by eliminating IPC overhead.

### agent

Communication with guest OS agent via ttRPC. Supports `KataAgent` for full container lifecycle management.

### persist

State serialization to disk for sandbox recovery after restart. Stores `state.json` under `/run/kata/<sandbox-id>/`.

## Build from Source and Install

### Prerequisites

Download `Rustup` and install Rust. For Rust version, see `languages.rust.meta.newest-version` in [`versions.yaml`](../../versions.yaml).

Example for `x86_64`:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
rustup install ${RUST_VERSION}
rustup default ${RUST_VERSION}-x86_64-unknown-linux-gnu
```

### Musl Support (Optional)

For fully static binary:

```bash
# Add musl target
rustup target add x86_64-unknown-linux-musl

# Install musl libc (example: musl 1.2.3)
curl -O https://git.musl-libc.org/cgit/musl/snapshot/musl-1.2.3.tar.gz
tar vxf musl-1.2.3.tar.gz
cd musl-1.2.3/
./configure --prefix=/usr/local/
make && sudo make install
```

### Install Kata 4.0 Rust Runtime Shim

```bash
git clone https://github.com/kata-containers/kata-containers.git
cd kata-containers/src/runtime-rs
make && sudo make install
```

After installation:
- Config file: `/usr/share/defaults/kata-containers/configuration.toml`
- Binary: `/usr/local/bin/containerd-shim-kata-v2`

### Install Without Built-in Dragonball VMM

To build without the built-in Dragonball hypervisor:

```bash
make USE_BUILTIN_DB=false
```

Specify hypervisor during installation:

```bash
sudo make install HYPERVISOR=qemu
# or
sudo make install HYPERVISOR=cloud-hypervisor
```

## Configuration

Configuration files in `config/`:

| Config File | Hypervisor | Notes |
|-------------|------------|-------|
| `configuration-dragonball.toml.in` | Dragonball | Built-in VMM |
| `configuration-qemu-runtime-rs.toml.in` | QEMU | Default external |
| `configuration-cloud-hypervisor.toml.in` | Cloud Hypervisor | Modern VMM |
| `configuration-fc-rs.toml.in` | Firecracker | Lightweight microVM |
| `configuration-remote.toml.in` | Remote | Remote hypervisor |
| `configuration-qemu-tdx-runtime-rs.toml.in` | QEMU + TDX | Intel TDX confidential computing |
| `configuration-qemu-snp-runtime-rs.toml.in` | QEMU + SEV-SNP | AMD SEV-SNP confidential computing |
| `configuration-qemu-se-runtime-rs.toml.in` | QEMU + SEV | AMD SEV confidential computing |
| `configuration-qemu-coco-dev-runtime-rs.toml.in` | QEMU + CoCo | CoCo development |

See [runtime configuration](../runtime/README.md#configuration) for configuration options.

## Logging

See [Developer Guide - Troubleshooting](../../docs/Developer-Guide.md#troubleshoot-kata-containers).

## Debugging

For development, use [`shim-ctl`](crates/shim-ctl/README.md) to test shim without containerd dependencies.

## Limitations

See [Limitations](../../docs/Limitations.md) for details.

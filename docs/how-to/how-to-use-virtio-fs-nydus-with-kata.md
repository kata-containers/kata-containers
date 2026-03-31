# Kata Containers with virtio-fs-nydus

## Introduction

Nydus is a container image acceleration service that provides fast container startup and on-demand data loading. Kata Containers integrates with Nydus through virtio-fs, supporting two operational modes:

### Operational Modes

1. **Standalone Mode (virtio-fs-nydus)**
   - Used with QEMU and Cloud-Hypervisor
   - Nydusd runs as an independent process
   - Supports native overlay filesystem directly in nydusd
   - Better performance with built-in overlay support

2. **Inline Mode (inline-virtio-fs) / Builtin Nydus**
   - Used with Dragonball VMM
   - Nydusd is built into the VMM (builtin nydus)
   - Overlay filesystem assembled by guest kernel
   - Suitable for lightweight VM scenarios
   - Lower resource overhead due to integrated architecture

Refer to [kata-nydus-design](../design/kata-nydus-design.md) for detailed design documentation.

## Architecture Overview

### Standalone Mode Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Host System                         │
│  ┌──────────────┐         ┌──────────────────────┐      │
│  │   nydusd     │◄────────┤  nydus-snapshotter   │      │
│  │  (standalone)│         │                      │      │
│  └──────┬───────┘         └──────────────────────┘      │
│         │ virtiofs                                       │
│         ▼                                                │
│  ┌──────────────┐                                       │
│  │     QEMU     │                                       │
│  │  / Cloud-Hypervisor                                  │
│  └──────────────┘                                       │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│                      Guest VM                            │
│  ┌──────────────────────────────────────────────┐       │
│  │  /run/kata-containers/shared/                 │       │
│  │    ├── containers/<cid>/rootfs  (overlay)     │       │
│  │    │     ├── upperdir                         │       │
│  │    │     ├── workdir                          │       │
│  │    │     └── lowerdir (from rafs)             │       │
│  │    └── rafs/<cid>/lowerdir (nydus image)      │       │
│  └──────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────┘
```

### Inline Mode (Builtin Nydus) Architecture

```
┌─────────────────────────────────────────────────────────┐
│                      Host System                         │
│  ┌──────────────────────────────────────────────┐       │
│  │          nydus-snapshotter                    │       │
│  └──────────────────────────────────────────────┘       │
│                                                          │
│  ┌──────────────────────────────────────────────┐       │
│  │              Dragonball VMM                   │       │
│  │  ┌────────────────────────────────────────┐  │       │
│  │  │     Builtin Nydusd (virtiofs server)    │  │       │
│  │  │  ┌──────────────────────────────────┐   │  │       │
│  │  │  │  Vfs (Virtual File System)       │   │  │       │
│  │  │  │  ├── Rafs (nydus image backend)  │   │  │       │
│  │  │  │  └── PassthroughFs               │   │  │       │
│  │  │  └──────────────────────────────────┘   │  │       │
│  │  └────────────────────────────────────────┘  │       │
│  └──────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────┘
         │
         ▼
┌─────────────────────────────────────────────────────────┐
│                      Guest VM                            │
│  ┌──────────────────────────────────────────────┐       │
│  │  /run/kata-containers/shared/containers/      │       │
│  │    ├── <cid>/rootfs          (overlay mount)  │       │
│  │    ├── <cid>/rootfs_lower/   (Rafs mount)     │       │
│  │    └── <cid>/snapshotdir/                     │       │
│  │          ├── fs/             (upperdir)       │       │
│  │          └── work/           (workdir)        │       │
│  │                                               │       │
│  │  Guest Kernel Overlay Assembly:               │       │
│  │    overlay lowerdir=rootfs_lower/             │       │
│  │           upperdir=snapshotdir/fs/            │       │
│  │           workdir=snapshotdir/work/           │       │
│  └──────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────┘
```

### Key Components

- **nydusd**: The nydus daemon that provides:
  - Virtiofs server for file sharing
  - Rafs (Registry Accelerated File System) image mounting
  - Native overlay filesystem support (standalone mode only)
  - Passthrough filesystem for host-guest file sharing

- **nydus-snapshotter**: Containerd snapshotter that:
  - Manages nydus image layers
  - Prepares container rootfs
  - Communicates with Kata runtime

- **Builtin Nydus (Inline Mode)**: Integrated nydus in Dragonball VMM:
  - Runs inside the VMM process, no separate daemon needed
  - Uses fuse_backend_rs Vfs to manage multiple filesystem backends
  - Supports Rafs for nydus image mounting
  - Supports PassthroughFs for host directory sharing
  - Lower memory footprint and faster startup
  - Overlay filesystem assembled by guest kernel instead of nydusd

## Prerequisites

### 1. Install Nydus

Use the [nydus latest branch](https://github.com/dragonflyoss/image-service) and build nydusd:

```bash
git clone https://github.com/dragonflyoss/image-service.git
cd image-service
make nydusd
```

### 2. Deploy Nydus Environment

Deploy nydus environment as described in [Nydus Setup for Containerd Environment](https://github.com/dragonflyoss/image-service/blob/master/docs/containerd-env-setup.md).

### 3. Start Nydus Snapshotter

Start `nydus-snapshotter` with `enable_nydus_overlayfs` enabled:

```bash
./nydus-snapshotter --enable-nydus-overlayfs
```

### 4. Build Kata Containers

Use [kata-containers](https://github.com/kata-containers/kata-containers) `latest` branch to compile and build `kata-containers.img`.

## Configuration

### Shared Filesystem Types

Kata Containers supports the following shared filesystem types:

| Type | Description | Hypervisor |
|------|-------------|------------|
| `virtio-fs` | Standard virtio-fs with virtiofsd | QEMU, Cloud-Hypervisor |
| `virtio-fs-nydus` | Virtio-fs with standalone nydusd | Dragonball, QEMU, Cloud-Hypervisor |
| `inline-virtio-fs` | Inline virtio-fs with builtin nydus | Dragonball |
| `none` | Disable shared filesystem | All |

### Configuration for QEMU

Update `configuration-qemu.toml` or `configuration-qemu-runtime-rs.toml`:

```toml
# Enable virtio-fs-nydus for standalone mode
shared_fs = "virtio-fs-nydus"

# Path to nydusd binary (required for virtio-fs-nydus)
virtio_fs_daemon = "/usr/local/bin/nydusd"

# Optional: Extra arguments for nydusd
# Example: virtio_fs_extra_args = ["--log-level", "debug", "--threads", "4"]
virtio_fs_extra_args = []

# Cache mode for virtio-fs (never, auto, always)
virtio_fs_cache = "never"

# Optional: Enable DAX for better performance
virtio_fs_is_dax = false
```

### Configuration for Cloud-Hypervisor

Update `configuration-clh.toml` or `configuration-cloud-hypervisor.toml`:

```toml
shared_fs = "virtio-fs-nydus"
virtio_fs_daemon = "/usr/local/bin/nydusd"
virtio_fs_extra_args = []
virtio_fs_cache = "never"
```

### Configuration for Dragonball (Inline Mode / Builtin Nydus)

For Dragonball VMM, use inline mode (builtin nydus), update `configuration-dragonball.toml`:

```toml
shared_fs = "inline-virtio-fs"
# Note: virtio_fs_daemon is not needed for inline mode
# nydusd is built into Dragonball VMM
virtio_fs_cache = "never"
```

## How Nydusd Works in Kata

### Nydusd Startup Process

#### Standalone Mode

When using `virtio-fs-nydus`, Kata runtime starts nydusd with the following parameters:

```bash
nydusd virtiofs \
  --hybrid-mode \
  --log-level info \
  --apisock /path/to/nydusd-api.sock \
  --sock /path/to/virtiofs.sock
```

Key features:

- **Hybrid Mode**: Enables both Rafs and passthrough filesystem support
- **API Socket**: Provides HTTP API for runtime to mount Rafs images
- **Virtiofs Socket**: Used by hypervisor for virtio-fs communication

After nydusd starts, Kata runtime automatically:

1. Waits for the nydusd API server to be ready
2. Mounts passthrough_fs at `/containers` within the nydusd virtiofs namespace
   - This maps to `/run/kata-containers/shared/containers/` in the guest
   - The passthrough_fs provides the writable layer for container overlay

#### Inline Mode (Builtin Nydus)

When using `inline-virtio-fs` with Dragonball:

- Nydusd is built into the Dragonball VMM binary
- No separate process or daemon startup required
- Virtiofs server initializes automatically during VMM boot
- Filesystem backends (Rafs, PassthroughFs) are mounted via VMM API
- Lower resource usage and faster initialization

### Filesystem Layout in Guest

#### Standalone Mode (virtio-fs-nydus)

```
/run/kata-containers/shared/                    # virtiofs mount point
├── containers/<container-id>/                  # passthrough_fs from host
│   ├── rootfs/                                 # container rootfs mount point
│   └── snapshotdir/                            # snapshot directory
│       ├── fs/                                 # upperdir (writable layer)
│       └── work/                               # workdir (overlay work directory)
└── rafs/<container-id>/lowerdir/               # Rafs mount (nydus image)
```

#### Inline Mode (inline-virtio-fs / Builtin Nydus)

```
/run/kata-containers/shared/containers/
├── <container-id>/
│   ├── rootfs/                                 # container rootfs mount point
│   ├── rootfs_lower/                           # Rafs mount (lowerdir)
│   └── snapshotdir/
│       ├── fs/                                 # upperdir
│       └── work/                               # workdir
└── passthrough/                                # passthrough filesystem
```

### Overlay Filesystem Assembly

#### Standalone Mode (Native Overlay in Nydusd)

Nydusd creates overlay filesystem internally with:

- **Lowerdir**: Rafs mount point (nydus image)
- **Upperdir**: Writable layer from snapshot directory
- **Workdir**: Overlay work directory

The overlay is created via nydusd API. The mount request body format:

```json
{
  "fs_type": "rafs",
  "source": "/path/to/bootstrap",
  "config": "{...nydus config...}",
  "overlay": "{\"upper_dir\": \"/run/kata-containers/shared/containers/<cid>/snapshotdir/fs\", \"work_dir\": \"/run/kata-containers/shared/containers/<cid>/snapshotdir/work\"}"
}
```

The `overlay` field is a JSON string containing:

- `upper_dir`: Path to the upper directory (writable layer) within the virtiofs namespace
- `work_dir`: Path to the work directory for overlay operations


#### Inline Mode (Guest Kernel Overlay)

The guest kernel assembles overlay filesystem:

- Kata agent receives overlay mount information
- Lowerdir points to Rafs mount (rootfs_lower/)
- Upperdir and workdir from snapshot directory
- No native overlay support in builtin nydusd

## Usage Examples

### Running Containers with nerdctl

```bash
$sudo nerdctl run --snapshotter nydus --runtime io.containerd.kata.v2 --net=none --rm -it ghcr.io/dragonflyoss/image-service/ubuntu:nydus-nightly-v5 lsblk
NAME      MAJ:MIN RM  SIZE RO TYPE MOUNTPOINTS
pmem0     259:0    0  254M  1 disk
`-pmem0p1 259:1    0  253M  1 part
```

### Running Containers with crictl

1. Create a sandbox configuration `nydus-sandbox.yaml`:

```yaml
meta
  attempt: 1
  name: nydus-sandbox
  uid: nydus-uid
  namespace: default
log_directory: /tmp
linux:
  security_context:
    namespace_options:
      network: 2
annotations:
  "io.containerd.osfeature": "nydus.remoteimage.v1"
```

2. Create a container configuration `nydus-container.yaml`:

```yaml
meta
  name: nydus-container
image:
  image: ghcr.io/dragonflyoss/image-service/ubuntu:nydus-nightly-v5
command:
  - /bin/sleep
args:
  - 600
log_path: container.1.log
```

3. Run the container:

```bash
crictl run -r kata nydus-container.yaml nydus-sandbox.yaml
```

### Using with Kubernetes

Create a Pod specification:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: nydus-test
  annotations:
    io.containerd.osfeature: "nydus.remoteimage.v1"
spec:
  runtimeClassName: kata
  containers:
  - name: test
    image: ghcr.io/dragonflyoss/image-service/ubuntu:nydus-nightly-v5
    command: ["/bin/sleep", "600"]
```

## Advanced Configuration

### Nydusd Extra Arguments

You can pass additional arguments to nydusd through configuration:

```toml
virtio_fs_extra_args = ["--log-level", "debug", "--threads", "4"]
```

### Prefetch Configuration

Enable prefetch for faster container startup:

```toml
# Path to prefetch file list
prefetch_list_path = "/path/to/prefetch_file.list"
```

### Performance Tuning

#### Cache Mode

Choose appropriate cache mode based on your use case:

- **never**: No caching, always fetch from host (default, safest)
- **auto**: Cache with timeout, good for read-heavy workloads
- **always**: Aggressive caching, best performance but may see stale data

```toml
virtio_fs_cache = "auto"
```

#### DAX (Direct Access)

Enable DAX for memory-mapped I/O:

```toml
virtio_fs_is_dax = true
virtio_fs_cache_size = 1024  # Size in MiB
```

## Debugging and Path Mappings

### Debug Logging

Enable debug logging for nydusd:

```toml
virtio_fs_extra_args = ["--log-level", "debug"]
```

Or via annotation:

```yaml
annotations:
  io.katacontainers.config.hypervisor.virtio_fs_extra_args: "--log-level=debug"
```

### Path Mappings

#### Standalone Mode (virtio-fs-nydus)

| Component | Host Path | Guest Path | Notes |
|-----------|-----------|------------|-------|
| Virtiofs mount | N/A | `/run/kata-containers/shared/` | Root virtiofs mount point |
| Passthrough FS | `/run/kata-containers/shared/sandboxes/<sid>/rw/` | `/run/kata-containers/shared/containers/` | Mounted at `/containers` in nydusd namespace |
| Rafs mount | Bootstrap path from snapshotter | `/run/kata-containers/shared/rafs/<cid>/lowerdir` | Mounted via nydusd API |
| Container rootfs | `/run/kata-containers/shared/sandboxes/<sid>/rw/<cid>/rootfs` | `/run/kata-containers/shared/containers/<cid>/rootfs` | Overlay mount point |
| Snapshot dir | From snapshotter | `/run/kata-containers/shared/containers/<cid>/snapshotdir/` | Contains upperdir and workdir |

#### Inline Mode (inline-virtio-fs)

| Component | Host Path | Guest Path | Notes |
|-----------|-----------|------------|-------|
| Virtiofs mount | N/A | `/run/kata-containers/shared/containers/` | Root virtiofs mount point |
| Passthrough FS | `/run/kata-containers/shared/sandboxes/<sid>/rw/passthrough/` | `/run/kata-containers/shared/containers/passthrough/` | Uses PASSTHROUGH_FS_DIR |
| Rafs mount | Bootstrap path from snapshotter | `/run/kata-containers/shared/containers/<cid>/rootfs_lower/` | Mounted via DeviceManager |
| Container rootfs | `/run/kata-containers/shared/sandboxes/<sid>/rw/passthrough/<cid>/rootfs` | `/run/kata-containers/shared/containers/<cid>/rootfs` | Overlay mount point |

## Comparison: Standalone vs Inline Mode

| Feature | Standalone (virtio-fs-nydus) | Inline (inline-virtio-fs) |
|---------|------------------------------|---------------------------|
| Hypervisor | QEMU, Cloud-Hypervisor | Dragonball |
| Nydusd Process | Independent process | Built into VMM |
| Overlay Support | Native (in nydusd) | Guest kernel |
| Performance | Better (native overlay) | Good |
| Resource Usage | Higher (separate process) | Lower (integrated) |
| Flexibility | More configurable | Less configurable |
| Use Case | General purpose | Lightweight VMs |
| Startup Time | Slower (daemon startup) | Faster (no daemon) |
| Memory Overhead | Higher | Lower |

## References

- [Nydus Image Service](https://github.com/dragonflyoss/image-service)
- [Nydus Setup for Containerd](https://github.com/dragonflyoss/image-service/blob/master/docs/containerd-env-setup.md)
- [Kata Containers with Nydus Design](../design/kata-nydus-design.md)
- [Virtio-fs Documentation](https://virtio-fs.gitlab.io/)

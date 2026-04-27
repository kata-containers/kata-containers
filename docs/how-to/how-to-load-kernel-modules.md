# Loading Custom Kernel Modules in Kata Containers

This document explains how to build, package, and deploy custom kernel
modules for Kata Containers guest VMs using the **kernel modules images**
feature.

> **Important: Your own custom modules will not work with official Kata kernel releases.**
>
> The official Kata kernel builds enforce module signature verification
> (`CONFIG_MODULE_SIG_FORCE=y`) and the signing key passphrase
> (`KBUILD_SIGN_PIN`) used during the build is **not public**. This means
> that modules you compile yourself **cannot be signed with the official
> key** and will be rejected by the released kernel at load time.
>
> To use custom kernel modules, you **must rebuild the Kata guest kernel**
> (and for confidential/TEE deployments, the entire stack: kernel,
> rootfs/initrd, and shim) using your own signing key. See
> [Security Considerations](#security-considerations) for details.

## Overview

By default, the Kata guest kernel is built without loadable module support
to keep the attack surface small and to simplify dm-verity measured boot.
When additional kernel modules are needed (e.g., hardware-specific drivers,
filesystem modules, or network drivers), the kernel modules images feature
allows attaching one or more separate disk images containing pre-compiled
modules to the guest VM.

Each disk image is cold-plugged as a virtio-blk block device, mounted
read-only inside the guest, and its modules made available through `depmod`
so that `modprobe` can find and load them.

### Architecture

```
Host                                   Guest VM
┌─────────────────────────┐           ┌─────────────────────────────────┐
│ configuration.toml      │           │                                 │
│ [[...kernel_modules_    │           │ /dev/vda ── rootfs (dm-verity)  │
│    images]]             │           │ /dev/vdb ── /run/kata-modules-0 │
│   path = "mlx5.img"     │──attach──▶│ /dev/vdc ── /run/kata-modules-1 │
│   path = "custom.img"   │──attach──▶│                                 │
│                         │           │ symlinks ──▶ /run/lib/modules/  │
└─────────────────────────┘           │ depmod -a -b /run ──▶ modprobe  │
                                      └─────────────────────────────────┘
```

## Prerequisites

1. **Enable kernel module loading** in the guest kernel by including the
   `modules/modules.conf` config fragment when building the kernel. For
   confidential (TEE) builds, the `signing/module_signing.conf` fragment
   is also included automatically.

2. **Build kernel modules** against the exact same kernel source tree and
   configuration used to build the Kata guest kernel.

3. **`build-modules-volume.sh`** script (in `tools/packaging/kernel/`) to
   package compiled modules into a disk image.

## Building a Modules Volume

After compiling your modules against the Kata guest kernel tree:

```bash
# Collect modules into a staging directory and create a tarball
STAGING=$(mktemp -d)
KVER=$(uname -r)  # or the version of your Kata guest kernel
mkdir -p "${STAGING}/lib/modules/${KVER}/extra"
cp /path/to/your/*.ko "${STAGING}/lib/modules/${KVER}/extra/"
tar -czf modules.tar.gz -C "${STAGING}" .

# Package into a disk image (optionally with dm-verity via -V)
./tools/packaging/kernel/build-modules-volume.sh \
    -m modules.tar.gz \
    -o /tmp/
```

The resulting `kata-modules-volume.img` is an ext4 disk image that can
optionally be protected with dm-verity by passing the `-V` flag.

## Configuration

### Direct TOML configuration

Add one or more `[[hypervisor.<name>.kernel_modules_images]]` entries
to the Kata runtime configuration file or a drop-in:

```toml
[[hypervisor.qemu.kernel_modules_images]]
path = "/opt/kata/share/kata-containers/mlx5-modules.img"
verity_params = ""

[[hypervisor.qemu.kernel_modules_images]]
path = "/opt/kata/share/kata-containers/custom-modules.img"
verity_params = "root_hash=abc123..."

[agent.kata]
kernel_modules = ["mlx5_core", "ntfs3"]
```

Each `kernel_modules_images` entry specifies:
- **`path`** -- Absolute path to the modules disk image on the host.
- **`verity_params`** -- Optional dm-verity parameters for integrity
  verification. Leave empty if the image is not verity-protected.

The `kernel_modules` list under `[agent.kata]` tells the agent which
modules to load via `modprobe` at sandbox creation time.

### Helm chart (kata-deploy)

When using the kata-deploy Helm chart, kernel module images are
configured per-shim in `values.yaml` under the `shims` section.
Each entry specifies the disk image path, optional dm-verity params,
and the list of module names to load:

```yaml
shims:
  qemu:
    kernelModulesImages:
      - path: "/opt/kata/share/kata-containers/kata-modules-ntfs.img"
        verityParams: ""
        modules:
          - ntfs3
      - path: "/opt/kata/share/kata-containers/kata-modules-mlx5.img"
        verityParams: ""
        modules:
          - mlx5_core
```

The Helm chart creates a ConfigMap with the image list, mounts it into
the kata-deploy pod, and generates a TOML drop-in file automatically
(including both `kernel_modules_images` and `kernel_modules` entries).

The images themselves must be present on each worker node at the
specified paths (e.g., via a DaemonSet, host provisioning, or a
shared filesystem).

Because module images are configured per-shim, incompatible kernel
variants (such as `qemu-nvidia-gpu`) simply do not have module images
configured, avoiding vermagic mismatches.

## How It Works

1. **Runtime** reads `kernel_modules_images` from configuration and
   calls `appendBlockImage()` for each, cold-plugging them as
   virtio-blk devices (vdb, vdc, ...).

2. **Runtime** creates `Storage` entries in the `CreateSandboxRequest`
   for each image, with mount points at `/run/kata-modules-0`,
   `/run/kata-modules-1`, etc.

3. **Agent** processes storages *before* loading kernel modules:
   - Creates a writable module tree under `/run/lib/modules/<version>/`
     (on tmpfs, since the rootfs is read-only).
   - Mounts each modules volume read-only and symlinks its contents
     into the `/run/lib/modules/<version>/` tree.
   - Runs `depmod -a -b /run` to rebuild the module dependency database.
   - Proceeds to load any kernel modules specified in
     `CreateSandboxRequest.kernel_modules` via `modprobe -d /run`.

## Security Considerations

### Which kernel builds enforce module signing?

Not all Kata kernel builds are the same. The table below shows which
builds include `CONFIG_MODULE_SIG_FORCE=y` (via the `-x` confidential
flag to `build-kernel.sh`), and therefore **require signed modules**:

| Kernel variant | Module signing enforced | Notes |
|---|---|---|
| `kernel` (default, x86_64/aarch64/s390x) | **Yes** | Built with `-x` (confidential) |
| `kernel-nvidia-gpu` | **Yes** | Built with `-x -g nvidia` |
| `kernel-debug` | No | Not built with `-x` |
| `kernel-dragonball-experimental` | No | Not built with `-x` |

The rootfs/initrd variants that pair with these kernels:

| Rootfs / initrd | Paired kernel | Signing enforced |
|---|---|---|
| `rootfs-image` / `rootfs-initrd` | `kernel` | **Yes** (on x86_64, aarch64, s390x) |
| `rootfs-image-confidential` / `rootfs-initrd-confidential` | `kernel` | **Yes** |
| `rootfs-image-nvidia-gpu` | `kernel-nvidia-gpu` | **Yes** |
| `rootfs-image-nvidia-gpu-confidential` | `kernel-nvidia-gpu` | **Yes** |

**In practice, nearly all production Kata deployments use a kernel
with `CONFIG_MODULE_SIG_FORCE=y`.** Only the debug and dragonball
experimental kernels skip it.

### Module signing and the `KBUILD_SIGN_PIN`

For all kernel builds listed as "Yes" above, the kernel **refuses to
load any module** whose signature does not match the signing key
embedded at build time. The passphrase that protects this signing key
(`KBUILD_SIGN_PIN`) is **not public and is never published** as part
of Kata releases.

**This is intentional.** If the `KBUILD_SIGN_PIN` were public, anyone
could sign arbitrary kernel modules that would be accepted by every
official Kata kernel, completely undermining module signature
verification.

As a consequence:

- **You cannot load custom-built modules on an official released Kata
  kernel.** The kernel will reject them because they are not signed with
  the official key.
- **You must rebuild the Kata guest kernel yourself**, using your own
  signing key and `KBUILD_SIGN_PIN`, and sign your modules with that
  same key during the kernel build.
- Your custom kernel must include the `modules/modules.conf` and
  `signing/module_signing.conf` config fragments.

### Official pre-built module images

The module images shipped by the Kata project (MLX5, NTFS3) are built
and signed within the same CI infrastructure that builds the official
kernel, using the same `KBUILD_SIGN_PIN`. **These images work
out-of-the-box** with the official released kernel -- no rebuild is
needed.

### Confidential Computing (CoCo) / TEE deployments

For CoCo/TEE deployments the situation is stricter. Because the
dm-verity root hash of the rootfs and the kernel binary are part of the
attestation chain, changing the kernel means you **must rebuild the
entire Kata stack**:

1. **Kernel** -- rebuilt with your own signing key and `KBUILD_SIGN_PIN`.
2. **Rootfs / initrd** -- rebuilt to include the new kernel's module
   verification certificate.
3. **Shim** -- rebuilt to embed the new dm-verity root hash.

This is by design: in the CoCo threat model, the trust boundary must
be fully controlled by the entity that performs attestation. Publishing
the signing key would allow anyone to inject arbitrary code into the
trusted guest, defeating attestation entirely.

### Non-confidential deployments

For non-confidential deployments using `kernel-debug` or
`kernel-dragonball-experimental` where `CONFIG_MODULE_SIG_FORCE` is
**not** enabled, pre-compiled unsigned modules can be loaded without
rebuilding the kernel, as long as they are built against the exact same
kernel version and configuration. Even in this case, using dm-verity
on the modules volume is strongly recommended.

### Kernel variant compatibility

Kernel modules carry a `vermagic` string that must match the running
kernel exactly. This string includes the kernel version and the
`CONFIG_LOCALVERSION` suffix. **Modules built against one kernel
variant will not load on another variant with a different
LOCALVERSION.**

The official pre-built module images (MLX5, NTFS3) are compiled
against the **default `kernel` variant**, which has no
`CONFIG_LOCALVERSION` set. They are compatible with:

| Kernel variant | Compatible | Reason |
|---|---|---|
| `kernel` (default) | **Yes** | Same LOCALVERSION (empty) |
| `kernel-nvidia-gpu` | **No** | LOCALVERSION is `-nvidia-gpu` |
| `kernel-debug` | **No** | Module signing not enforced, but vermagic may differ |
| `kernel-dragonball-experimental` | **No** | Different build type |

For `kernel-nvidia-gpu` specifically:

- The nvidia-gpu kernel already bundles MLX5/InfiniBand modules
  in-tree as part of its build, so separate MLX5 module images are
  typically not needed.
- Because `kernelModulesImages` is configured **per-shim** in the
  Helm chart, simply do not add module images to the nvidia-gpu shim
  entries to avoid incompatibilities.
- If you need custom modules for the nvidia-gpu kernel, you must
  build them against that kernel variant specifically.

### Integrity protection

Use dm-verity on modules volumes to ensure their contents have not been
tampered with. The `verity_params` configuration field carries the root
hash and related parameters for runtime verification.

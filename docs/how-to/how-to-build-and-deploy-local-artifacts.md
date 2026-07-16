# Building and Deploying Local Kata Artifacts

## Overview

This guide shows how to build Kata guest artifacts (the guest **kernel**,
**rootfs image**, and **shim**) locally, deploy them to a test node, and run
them with a custom runtime config.

It also documents two failure modes that are easy to hit and hard to diagnose:
a rootfs image that builds "successfully" but is empty, and a guest kernel
panic caused by module signature enforcement. Both are covered in
[Troubleshooting](#troubleshooting).

!!! note

    Examples use the **NVIDIA GPU** variant (`nvidia-gpu`), which exercises
    EROFS, measured rootfs (dm-verity), and signed modules. Substitute the
    variant name for others.

## Build system

Artifacts are built from `tools/packaging/kata-deploy/local-build/`. Each
target produces a `kata-static-<component>.tar.zst` under `build/`:

```bash
cd tools/packaging/kata-deploy/local-build

make kernel-nvidia-gpu-tarball          # guest kernel + modules
make rootfs-image-nvidia-gpu-tarball    # rootfs image (builds the kernel first)
make shim-v2-tarball                    # shim + configs
```

### USE_CACHE

`USE_CACHE` defaults to `yes`, which pulls prebuilt artifacts from CI and skips
the local build whenever a cached version matches. This means a `make` run may
not contain your changes, and it mixes CI components with your own. Force a
local build with:

```bash
export USE_CACHE=no
make rootfs-image-nvidia-gpu-tarball
```

!!! note

    `build/<component>-version`, `-sha256sum`, and `-builder-image-version` files
    are the fingerprint of a cache pull. If you intended a local build and see
    them, you forgot `USE_CACHE=no`.

The GPU image build sets `FS_TYPE=erofs`, `MEASURED_ROOTFS=yes`, and
`SKIP_DAX_HEADER=yes` automatically.

## Rebuild only the rootfs image

When the rootfs contents are unchanged and you only need to repackage them
(for example after editing `image_builder.sh`), build the image directly
against the populated rootfs directory:

```bash
cd tools/osbuilder/image-builder

ROOTFS=../../packaging/kata-deploy/local-build/build/rootfs-image-nvidia-gpu/builddir/rootfs-image/ubuntu_rootfs

sudo USE_DOCKER=1 \
  FS_TYPE=erofs \
  MEASURED_ROOTFS=yes \
  SKIP_DAX_HEADER=yes \
  BUILD_VARIANT=nvidia-gpu \
  AGENT_INIT=no \
  ./image_builder.sh -o /tmp/kata-rootfs.image "$ROOTFS"
```

`USE_DOCKER=1` runs the build in the `image-builder-osbuilder` container, which
has the required tools and bind-mounts the on-disk `image_builder.sh`, so local
edits take effect.

Always verify the image is populated (see
[Verifying an image is populated](#verifying-an-image-is-populated)) before
deploying.

## Deploy to a node

Artifacts install under `/opt/kata/share/kata-containers/`.

### Replace the default artifacts

Extract the kernel tarball at `/` so its paths land in place and the
`*-nvidia-gpu.container` symlinks repoint to the new kernel:

```bash
sudo tar --zstd -xvf kata-static-kernel-nvidia-gpu.tar.zst -C /
cp kata-rootfs.image /opt/kata/share/kata-containers/<your-image-name>.image
```

### Use a custom location

To leave the shipped artifacts untouched, extract into a side directory and
point a custom runtime config at it:

```bash
sudo mkdir -p /opt/kata/share/kata-containers/custom/
sudo tar --zstd -xvf kata-static-kernel-nvidia-gpu.tar.zst \
  -C /opt/kata/share/kata-containers/custom/
```

```toml
[hypervisor.qemu]
image  = "/opt/kata/share/kata-containers/<your-image-name>.image"
kernel = "/opt/kata/share/kata-containers/custom/opt/kata/share/kata-containers/vmlinux-nvidia-gpu.container"
# Disable dm-verity so rebuilt test images boot (see Troubleshooting).
kernel_verity_params = ""
```

> **Important:**
>
> The kernel and rootfs must come from the **same build**. The rootfs ships the
> kernel modules under `/lib/modules/<version>`, which must match the running
> kernel's version and, if enforcement is on, its signing key. Deploy them as a
> pair.

After deploying, recreate the pod. The kernel, image, and `kernel_verity_params`
are read at sandbox creation, so a running sandbox keeps the old values.

## Troubleshooting

### The rootfs image is empty

**Symptom.** The guest panics early:

```
VFS: Unable to mount root fs on unknown-block(...)
erofs (device ...): cannot read erofs superblock
```

and the firmware falls back to PXE. A hexdump shows a valid partition table but
zeros where the filesystem should be.

**Cause.** `image_builder.sh` writes the filesystem into the image through a
loop partition device (`dd ... of=${device}p1`). In nested or unprivileged
containers that node does not map to the backing file, so the write silently
lands nowhere and its exit status is not checked. You get a valid, even
dm-verity-signed, image over an empty partition, and the build reports success.

**Diagnose.** A healthy EROFS image has magic `e2 e1 f5 e0` at offset
`0x100400`; an empty one is all zeros from `0x200` to EOF. A
`kata-static-rootfs-image-*.tar.zst` of only tens of KB (566 MB of zeros
compresses to almost nothing) is another giveaway.

**Fix.** Make the write independent of loop partition devices:

- Build where loop partitions work, such as a `--privileged` container with
  `-v /dev:/dev`, or directly on a host; or
- Use a loopless image build that writes the filesystem into the backing file
  at the partition offset
  (`dd if=<fsimage> of=<image> bs=1M seek=<rootfs_start> conv=notrunc,fsync`)
  and runs `veritysetup format` on plain files instead of partition devices.

### dm-verity panic: "metadata block 0 is corrupted"

**Symptom.**

```
device-mapper: verity: ...: metadata block 0 is corrupted
erofs (device dm-0): cannot read erofs superblock
```

**Cause.** The root hash on the kernel command line does not match the hash
tree in the image. The runtime reads `kernel_verity_params` from
`configuration-*.toml`, not from `root_hash_<variant>.txt` at boot. That value
is baked into the config at shim build time, so rebuilding the image without
rebuilding the config leaves them mismatched. Each rebuild also uses a new
random salt, so the hash changes every time.

**Fix.**

- Disable verity for testing: set `kernel_verity_params = ""` (empty string and
  removing the key are equivalent). The runtime then boots `root=/dev/vda1` and
  ignores the hash partition.
- Or keep it in sync: rebuild the image and shim/config together and deploy
  them as a set, so `kernel_verity_params` equals the build's
  `root_hash_<variant>.txt`.

### Kernel panic: "Loading of module with unavailable key is rejected"

**Symptom.** The rootfs mounts and init runs, then:

```
Loading of module with unavailable key is rejected
panic: /sbin/modprobe failed with status: exit status: 1
```

**Cause.** Module signature enforcement. Built with `KBUILD_SIGN_PIN` set, the
kernel gets `CONFIG_MODULE_SIG_FORCE=y` and signs modules with a key derived
from that pin, and `CONFIG_SYSTEM_TRUSTED_KEYS` is empty, so it trusts only its
own key. "unavailable key" means the module is signed with a key the running
kernel does not trust, i.e. the kernel and the in-rootfs modules came from
different builds. This often comes from `USE_CACHE=yes` pulling a CI kernel
while the modules were signed by another build.

**Fix.**

- Build the kernel and rootfs together with `USE_CACHE=no` so they share one
  key, and deploy both.
- Or build without enforcement (simplest for testing): a kernel built without
  `KBUILD_SIGN_PIN` has no `MODULE_SIG_FORCE` and loads modules regardless of
  key. Verify with:

  ```bash
  tar --zstd -xO -f kata-static-kernel-nvidia-gpu.tar.zst \
    ./opt/kata/share/kata-containers/config-*-nvidia-gpu | grep MODULE_SIG_FORCE
  # want: # CONFIG_MODULE_SIG_FORCE is not set
  ```

!!! note

    Even with signing off, the kernel and modules must share the same version
    (`vermagic`, e.g. `6.18.28-nvidia-gpu`) or `modprobe` fails. Same-build
    artifacts guarantee this.

### Boots straight to UEFI/PXE with no kernel output

**Symptom.** The guest produces no kernel log lines at all and the firmware
loops on boot-device selection and PXE:

```
BdsDxe: failed to load Boot0002 "UEFI Misc Device" ...: Not Found
>>Start PXE over IPv4.
BdsDxe: No bootable option or device was found.
```

**Cause.** `kernel =` points at `vmlinux` (the raw ELF) instead of `vmlinuz`
(the bzImage). Variants that boot through OVMF firmware (`firmware = .../OVMF.fd`
in the config, `-bios .../OVMF.fd` in the qemu line) require a `vmlinuz`
bzImage; OVMF cannot boot a raw `vmlinux` ELF. QEMU loads `-kernel` into fw_cfg
without validating it, so the VM starts, OVMF fails to boot the kernel, and
falls through to disk/PXE. The default GPU config uses
`vmlinuz-nvidia-gpu.container` for this reason; a custom override that sets
`vmlinux` instead hits this.

**Fix.** Point `kernel =` at the `vmlinuz` symlink:

```toml
kernel = "/opt/kata/share/kata-containers/vmlinuz-nvidia-gpu.container"
```

The kernel tarball ships both `vmlinux-*` and `vmlinuz-*` plus their
`.container` symlinks, so this is a config-only change. Confirm the qemu command
line then shows `-kernel .../vmlinuz-...`.

## Verifying an image is populated

```bash
python3 - /path/to/your.image <<'EOF'
import sys
f = open(sys.argv[1], 'rb'); f.seek(0x100400); d = f.read(16)
print("0x100400:", d.hex(' '))
print("EROFS magic present:", d[:4] == bytes.fromhex('e2e1f5e0'))
EOF
```

`EROFS magic present: True` means the filesystem is really in the image.

## NVIDIA devkit extension

The production NVIDIA base image ships a **shell-less** busybox (minimal attack
surface), so the agent debug console has no interactive shell on the guest root.

Instead of rebuilding the whole NVIDIA rootfs with a debug busybox, build the
optional **devkit** guest extension and cold-plug it alongside the nvidia
base and gpu extension.  The devkit extension is a self-contained minimal
**Alpine** rootfs (musl + busybox + apk) with common debug tools prebaked
(strace, gdb, ltrace, iproute2, procps, lsof, tcpdump, ...), plus a `bin/sh`
wrapper over static busybox.  At runtime it overlays a writable tmpfs and
chroots in, so `apk` runs natively; use `devkit-apk add <pkg>` to install
anything else into the writable overlay.

!!! note
    Alpine is used (rather than an Ubuntu/glibc rootfs) so its musl sonames
    never collide with the base image's glibc — mounting the extension cannot
    shadow or corrupt the base kata-agent — and to avoid dpkg maintainer-script
    fragility. It is not tied to any CUDA/apt repository.

```bash
cd tools/packaging/kata-deploy/local-build

export USE_CACHE=no
make rootfs-image-nvidia-devkit-extension-tarball
```

The resulting `kata-static-rootfs-image-nvidia-devkit-extension.tar.zst` contains
`kata-containers-nvidia-devkit-extension.img` and `root_hash_nvidia-devkit-extension.txt` (dm-verity
parameters for the extension), an EROFS image with the Alpine rootfs and `devkit-apk`
wrapper mounted at `/run/kata-extensions/devkit/` inside the guest.

Build the devkit extension together with the NVIDIA runtime-rs shim so the
root hash stays in sync (the shim build reads `root_hash_nvidia-devkit-extension.txt` from
`tools/packaging/kata-deploy/local-build/build/` when present).

### Deploy and use

Extract the tarball under `/opt/kata/share/kata-containers/` (or your custom
prefix).

Enable the debug console and cold-plug the extension with a **drop-in** under
the runtime config's `config.d/` directory. The same fragment works for every
NVIDIA runtime-rs profile (`qemu-nvidia-cpu`, `qemu-nvidia-gpu`,
`qemu-nvidia-gpu-snp`, `qemu-nvidia-gpu-tdx`); `guest_extension_images` entries
from drop-ins are appended to the base config.

=== "Manual drop-in"

    Copy the example shipped with runtime-rs (after `make install` or from the
    source tree):

    ```bash
    example=/opt/kata/share/defaults/kata-containers/runtime-rs/drop-in-examples/25-devkit.toml.example
    cfg=/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/qemu-nvidia-gpu-runtime-rs/configuration-qemu-nvidia-gpu-runtime-rs.toml
    verity="$(tr -d '\n' < /opt/kata/share/kata-containers/root_hash_nvidia-devkit-extension.txt)"
    sudo install -d "$(dirname "$cfg")/config.d"
    sudo cp "${example}" "$(dirname "$cfg")/config.d/25-devkit.toml"
    sudo sed -i "s|verity_params = .*|verity_params = \"${verity}\"|" \
      "$(dirname "$cfg")/config.d/25-devkit.toml"
    ```

    Adjust `path` if your install prefix is not `/opt/kata`. The `verity_params`
    value must match `root_hash_nvidia-devkit-extension.txt` from the same build as the
    `.img` file (NVRC requires measured extensions).

=== "kata-deploy"

    Set `debug: true` in the kata-deploy Helm values (or environment). For
    NVIDIA runtime-rs shims, kata-deploy writes `20-debug.toml` with
    `debug_console_enabled = true` and the devkit extension entry (including
    `verity_params` from `root_hash_nvidia-devkit-extension.txt`) when that file is
    installed under `share/kata-containers/`.

Example drop-in contents:

```toml title="config.d/25-devkit.toml"
[agent.kata]
debug_console_enabled = true

[[hypervisor.qemu.guest_extension_images]]
name = "devkit"
path = "/opt/kata/share/kata-containers/kata-containers-nvidia-devkit-extension.img"
verity_params = "root_hash=...,salt=...,data_blocks=...,data_block_size=...,hash_block_size=..."
```

Copy the `verity_params` value verbatim from
`/opt/kata/share/kata-containers/root_hash_nvidia-devkit-extension.txt`.

!!! warning
    The devkit extension is for **development and troubleshooting only**.
    Do not include it in attested production or TEE runtime configurations.

!!! note
    Rebuild and redeploy the devkit extension image whenever its contents change;
    the dm-verity root hash in `root_hash_nvidia-devkit-extension.txt` must match the
    image or NVRC will refuse to mount the extension at boot.

### Open a shell and install packages with devkit-apk

Enable the devkit drop-in (`25-devkit.toml.example`) or set:

```toml
[agent.kata]
debug_console_enabled = true
debug_console_shell = "/run/kata-extensions/devkit/bin/sh"
```

That passes `agent.debug_console_shell` on the guest kernel command line. Then connect
with the debug console (no command argument — the agent spawns the configured shell):

```bash
kata-runtime exec <sandbox-id>
```

The devkit `bin/sh` wrapper enters the overlay/chroot and drops you into a
bash shell inside the Alpine rootfs, with all prebaked debug tools on `PATH`.

The guest rootfs and devkit extension are read-only (EROFS + dm-verity). The
`devkit-apk` wrapper (and any `apk` call from inside the debug shell) installs
packages into a writable tmpfs overlay at `/run/kata-devkit-writable/`:

```bash
# strace, gdb, iproute2, ... are already prebaked and work offline:
strace --version

# install anything else at runtime (needs guest network):
devkit-apk add htop
```

Package sources are the standard Alpine `main`/`community` repositories pinned in
`versions.yaml` (`externals.alpine`). Packages installed at runtime and the apk
state live under `/run/kata-devkit-writable/` and are lost when the pod/guest is
recreated.

# Composable VM Images for Kata Containers

> **Status**: Proposal
>
> Once accepted and implemented, this document should be moved to
> `docs/design/composable-vm-images.md`.

## Summary

This proposal introduces a **composable image architecture** for Kata Containers
guest VMs. Instead of building a single monolithic rootfs image that contains
every component a workload might need, the runtime assembles a VM from a lean
**base image** plus zero or more purpose-specific **addon images** that are
cold-plugged as additional virtio-blk devices. Each addon is an EROFS image
protected by dm-verity, mounted read-only inside the guest before the kata-agent
starts.

The first application of this architecture splits **Confidential Containers
(CoCo) guest components** out of the monolithic `kata-containers-confidential.img`
into a separate `kata-containers-coco-addon.img`.

The current proposal targets **QEMU** as the hypervisor backend. The
design is intentionally hypervisor-agnostic at the configuration and guest
layers — the `extra_images` configuration, guest-side systemd units, and
agent path resolution work identically regardless of the hypervisor. Only the
host-side device attachment is hypervisor-specific (QEMU virtio-blk
cold-plug today), and extending support to other hypervisor backends
(Cloud Hypervisor, Firecracker, etc.) requires only implementing the
equivalent block device attachment in each backend.

## Motivation

Today every CoCo runtime class ships a self-contained rootfs image that bundles
the base OS, the kata-agent, **and** every CoCo-specific binary
(attestation-agent, confidential-data-hub, api-server-rest, ocicrypt config,
pause bundle). This creates several problems:

1. **Image bloat** — the confidential image is significantly larger than the
   standard image because it carries components that are only needed for
   confidential workloads.

2. **Combinatorial explosion** — adding new feature dimensions (e.g. GPU
   support, different TEE backends) multiplies the number of monolithic images
   that must be built, tested, and distributed.

3. **Slow iteration** — updating a single CoCo binary requires rebuilding and
   re-signing the entire rootfs image.

4. **Lack of composability** — users who need a custom combination of features
   must maintain their own image build pipeline.

A composable approach addresses all four issues: the base image stays small and
generic, addon images are independently built and versioned, and the runtime
composes them at VM creation time.

## Design

### Architecture overview

```
Host                                     Guest VM
┌──────────────────────┐        ┌─────────────────────────────────┐
│  Runtime config      │        │  /  (base rootfs, erofs)        │
│  ┌────────────────┐  │        │    kata-agent, systemd, ...     │
│  │ image = base   │  │  boot  │                                 │
│  │ extra_images:  │──────────>│  /run/kata-addons/coco/ (erofs) │
│  │  - name: coco  │  │  cold  │    attestation-agent            │
│  │    path: ...   │  │  plug  │    confidential-data-hub        │
│  │    verity: ... │  │        │    api-server-rest              │
│  └────────────────┘  │        │    ocicrypt_config.json         │
│                      │        │    pause_bundle/                │
│  QEMU                │        │                                 │
│    -drive base.img   │        │  dm-verity protects both the    │
│    -drive coco.img   │        │  base rootfs and the addon.     │
│      serial=addon-…  │        └─────────────────────────────────┘
└──────────────────────┘
```

### Runtime configuration

A new `extra_images` field on the hypervisor configuration accepts an ordered
list of addon images:

```toml
[hypervisor.qemu]
image = "/opt/kata/share/kata-containers/kata-containers.img"

# ...existing keys...

[[hypervisor.qemu.extra_images]]
name = "coco"
path = "/opt/kata/share/kata-containers/kata-containers-coco-addon.img"
verity_params = "root_hash=abc...,salt=def...,data_blocks=1234,hash_block_size=4096,data_block_size=4096"
```

Each entry maps to a Rust struct:

```rust
pub struct ExtraImage {
    pub name: String,
    pub path: String,
    pub verity_params: String,
}
```

The `name` field is the primary identifier for an addon. It must be unique
within the configuration and is used to:

- Set the **virtio-blk serial** to `addon-<name>`, enabling deterministic
  device discovery in the guest via `/sys/block/*/serial`.
- Namespace the **kernel command-line** verity parameters as
  `kata.addon.<name>.verity_params=...`.
- Name the guest **mount point** at `/run/kata-addons/<name>/`.
- Name the **dm-verity device mapper target** as `addon-<name>`.

### Host-side: cold-plugging addon devices

At VM creation time, for each entry in `extra_images`, the runtime:

1. Creates a virtio-blk device backed by the addon image file.
2. Sets the device serial to `addon-<name>`.
3. If `verity_params` is non-empty, appends
   `kata.addon.<name>.verity_params=<value>` to the guest kernel command line.

Both the Go runtime and the Rust runtime-rs implement this logic in their
respective QEMU hypervisor backends. The addon devices are **cold-plugged** —
they are present on the QEMU command line at VM start, not hot-plugged later.

Other hypervisor backends (Cloud Hypervisor, Firecracker) do not implement
addon cold-plugging yet, but the mechanism is straightforward to add: each
backend only needs to translate the `ExtraImage` entries into its native
block device attachment API. The runtime configuration, guest-side mounting,
and agent path resolution are completely hypervisor-independent.

### Guest-side: mounting addons

A systemd template unit `kata-addon-mount@.service` handles addon discovery and
mounting. The unit is instantiated per addon name (e.g.
`kata-addon-mount@coco.service`).

The mounting is performed by a systemd unit (before the agent starts) rather
than by the kata-agent itself for several reasons:

- **Chicken-and-egg problem** — in the CoCo addon case, the addon carries
  the very guest components (attestation-agent, confidential-data-hub) that
  the kata-agent needs to launch. The agent cannot mount the addon because
  it needs the addon's contents to be available before it starts.

- **Init-system ordering guarantees** — systemd provides declarative
  ordering (`Before=`, `After=`), conditional activation
  (`ConditionKernelCommandLine=`), and failure handling
  (`OnFailure=poweroff.target`). Reimplementing these guarantees inside the
  agent would duplicate init-system responsibilities.

- **Separation of concerns** — block device discovery, dm-verity setup, and
  filesystem mounting are OS-level operations that belong in the init layer.
  The agent's role is to consume the mounted filesystem, not to manage block
  devices.

- **Non-systemd environments** — the design does not mandate systemd. In
  environments using a different init system (or a dedicated orchestrator
  like NVRC), the equivalent mounting logic can be implemented by whatever
  entity manages early boot. The key requirement is that addon images are
  mounted before the kata-agent starts — how that is achieved is an
  init-system concern.

#### Service unit

```ini
[Unit]
Description=Mount Kata addon image %i
DefaultDependencies=no
Before=kata-agent.service
After=local-fs-pre.target
ConditionKernelCommandLine=kata.addon.%i.verity_params
OnFailure=poweroff.target

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/libexec/kata-addon-mount.sh %i
ExecStop=/usr/libexec/kata-addon-umount.sh %i

[Install]
WantedBy=kata-containers.target
```

Key design decisions:

- **`ConditionKernelCommandLine`** — the service only activates when the
  runtime has actually configured the addon. This prevents the unit from
  running (and failing) in non-addon VM configurations.

- **`Before=kata-agent.service`** — guarantees the addon filesystem is mounted
  before the agent attempts to use any component from it.

- **`OnFailure=poweroff.target`** — if the addon cannot be mounted (e.g.
  verity verification failure, missing device), the VM is shut down
  immediately. A confidential VM must not continue with an unverified or
  missing addon.

#### Mount script

The mount script (`kata-addon-mount.sh`) performs the following steps:

1. **Device discovery** — scans `/sys/block/*/serial` for a device whose
   serial matches `addon-<name>`. This is more reliable than waiting for udev
   to create `/dev/disk/by-id/` symlinks, since the minimal guest environment
   may not run a full udev daemon.

2. **Verity parameter extraction** — reads
   `kata.addon.<name>.verity_params=...` from `/proc/cmdline` and parses the
   comma-separated key=value pairs (root_hash, salt, data_blocks,
   hash_block_size, data_block_size).

3. **dm-verity setup** — runs `veritysetup open` with `--no-superblock` to
   open the verity target. The `--no-superblock` flag is required because the
   image builder uses `veritysetup format --no-superblock` during image
   creation — all verity parameters are passed explicitly rather than stored
   in an on-disk superblock.

4. **EROFS mount** — mounts the verified device (or the raw partition if
   verity is not configured) read-only at `/run/kata-addons/<name>/`.

### Agent-side: dynamic path resolution

The kata-agent resolves CoCo guest component paths at runtime rather than
relying on hardcoded legacy paths. For each component, the agent checks the
addon mount path first, then falls back to the legacy location:

| Component              | Addon path                                              | Legacy path                     |
|------------------------|---------------------------------------------------------|---------------------------------|
| attestation-agent      | `/run/kata-addons/coco/usr/local/bin/attestation-agent` | `/usr/local/bin/attestation-agent` |
| confidential-data-hub  | `/run/kata-addons/coco/usr/local/bin/confidential-data-hub` | `/usr/local/bin/confidential-data-hub` |
| api-server-rest        | `/run/kata-addons/coco/usr/local/bin/api-server-rest`   | `/usr/local/bin/api-server-rest` |
| ocicrypt_config.json   | `/run/kata-addons/coco/etc/ocicrypt_config.json`        | `/etc/ocicrypt_config.json`     |
| pause_bundle           | `/run/kata-addons/coco/pause_bundle`                    | `/pause_bundle`                 |

This dual-path resolution approach:

- Preserves **backward compatibility** with existing monolithic rootfs images
  where CoCo components are baked into the base image.
- Requires **no special rootfs modifications** — the base image does not need
  stub files or directories for the addon components.
- Works transparently on a **read-only rootfs** — no bind-mounting, no
  remounting, no writes to the root filesystem.

### Image build pipeline

#### Base image

The standard `kata-containers.img` is built as before, but **without** CoCo
guest components (attestation-agent, confidential-data-hub, api-server-rest,
ocicrypt config, pause bundle). It includes:

- The kata-agent
- systemd and the `kata-addon-mount@.service` template unit
- `cryptsetup-bin` (provides `veritysetup`) — required unconditionally so
  that the base image can mount verity-protected addons regardless of whether
  the base itself was built with `CONFIDENTIAL_GUEST=yes`.

#### CoCo addon image

The `kata-containers-coco-addon.img` is built by:

1. Unpacking the CoCo guest components tarball into a temporary rootfs.
2. Unpacking the pause image tarball into the same rootfs.
3. Running the image builder with:
   - `FS_TYPE=erofs` — EROFS filesystem for compact, read-only storage.
   - `MEASURED_ROOTFS=yes` — creates a dm-verity hash partition.
   - `SKIP_DAX_HEADER=yes` — no DAX header (virtio-blk, not NVDIMM).
   - `SKIP_ROOTFS_CHECK=yes` — the addon has no `/sbin/init`.
   - `BUILD_VARIANT=coco-addon` — produces a correctly named root hash file
     (`root_hash_coco-addon.txt`).

The resulting image is a two-partition disk:

- **Partition 1**: EROFS data partition containing the CoCo components.
- **Partition 2**: dm-verity hash partition.

The root hash and verity parameters are captured at build time and injected
into the runtime configuration templates.

### Security model

The addon architecture preserves the existing security guarantees of
Confidential Containers:

- **dm-verity** provides integrity protection for the addon image, identical
  to the existing protection on the base rootfs. Any tampering with the addon
  contents is detected at mount time.

- **Verity parameters on the kernel command line** are measured by the TEE
  firmware (OVMF/TDVF for TDX, SEV-SNP firmware) as part of the launch
  measurement. An attacker cannot substitute different verity parameters
  without changing the measurement, which would be detected during
  attestation.

- **Mount failure = VM shutdown** — `OnFailure=poweroff.target` ensures the
  VM does not proceed with missing or tampered components.

- **Read-only mounts** — both the base rootfs and addon images are mounted
  read-only (EROFS), preventing runtime modification.

## Alternatives considered

### systemd-sysext

systemd provides a built-in mechanism for composable system extensions via
[systemd-sysext](https://www.freedesktop.org/software/systemd/man/latest/systemd-sysext.html).
System extension images are overlaid onto `/usr/` and `/opt/` using
overlayfs, making their contents appear as if they were part of the base OS
image. The mechanism supports EROFS, squashfs, and ext4 images, and can
verify them with dm-verity.

sysext was evaluated and is conceptually aligned with this proposal.
However, several gaps make it unsuitable as a direct replacement for the
custom mounting approach described here:

- **Hierarchy coverage** — sysext only merges into `/usr/` and `/opt/`.
  Files under `/etc/` (e.g. `ocicrypt_config.json`) require the separate
  `confext` mechanism, and files outside these three hierarchies (e.g.
  `/pause_bundle/`) cannot be delivered via sysext or confext at all.

- **Block device bridging** — sysext discovers images from well-known
  directories (`/var/lib/extensions/`, `/run/extensions/`), not from raw
  block devices. A bridging step would still be needed to discover the
  virtio-blk addon device, verify it with dm-verity, and place or symlink it
  where sysext expects to find it — eliminating much of the simplification
  sysext would offer.

- **Boot ordering** — `systemd-sysext.service` merges extensions before
  `basic.target`, but the precise ordering relative to `kata-agent.service`
  is not directly controllable. The current `Before=kata-agent.service`
  guarantee on the custom mount unit gives explicit control over this.

- **Version matching** — sysext enforces `extension-release.d` metadata
  matching against the host `os-release`. While this is useful for general
  purpose systems, it adds friction in the Kata context where the base image
  and addon images are built and versioned together in a controlled pipeline.

- **Minimal guest environment** — the Kata guest rootfs is a minimal
  environment that may not ship a full systemd with sysext/confext support
  enabled.

The current proposal can be evolved toward sysext in the future if these
gaps are addressed, particularly if sysext gains support for block-device-backed
extensions or if the Kata guest components are restructured to fit entirely
within `/usr/`.

### bootc and ostree

[bootc](https://github.com/containers/bootc) and
[ostree](https://ostreedev.github.io/ostree/) provide image-based OS
deployment and update mechanisms. bootc in particular enables building
bootable container images and managing OS updates as container image pulls.

While bootc and ostree share the high-level goal of composable, image-based
OS management, they solve a fundamentally different problem than this
proposal:

- **bootc/ostree manage the base OS lifecycle** — they address how the
  root filesystem is built, deployed, and updated over time (e.g. atomic
  upgrades, rollbacks, image pulls).

- **This proposal manages VM-time composition** — it addresses how
  purpose-specific components are attached to an already-built base image
  at VM creation time, without modifying the base image itself.

The two approaches are orthogonal. A Kata base image could be built and
managed using bootc/ostree, and addon images would still be cold-plugged
and mounted at VM boot using the mechanism described here. bootc does not
provide a mechanism for dynamically composing additional block devices into
a running or booting system — it operates at the image build and deployment
layer, not at the VM assembly layer.

## Future work

### Additional addon types

The architecture is designed to support multiple addon images. Planned
extensions include:

- **GPU addon** — NVIDIA GPU support components (NVRC, vGPU drivers)
  in a separate addon image, enabling GPU support without bloating
  the base or CoCo images.

- **Custom addons** — users can build their own addon images for
  workload-specific libraries, models, or configurations.

### Addon ordering

When multiple addons are configured, they are cold-plugged in the order they
appear in the `extra_images` list. The current design does not enforce
explicit ordering dependencies between addons. If future use cases require
addon-to-addon dependencies, the systemd units can be extended with
appropriate `After=`/`Requires=` relationships.

### Other hypervisor backends

The current proposal covers QEMU only. Extending to other backends
requires implementing block device cold-plug for each:

- **Cloud Hypervisor** — add `--disk` entries with the addon image path and
  serial. Cloud Hypervisor natively supports virtio-blk serial numbers.
- **Dragonball** — attach additional virtio-blk devices through the
  Dragonball VMM's block device configuration, mapping each `ExtraImage`
  to a drive with the corresponding serial.
- **Firecracker** — add block device entries via the Firecracker API with
  the appropriate drive ID. Serial-based discovery may need adaptation since
  Firecracker exposes drive IDs differently.

No changes are needed in the guest-side mounting logic or the agent — the
addon device discovery via `/sys/block/*/serial` and the systemd units work
the same way regardless of which hypervisor attached the block device.

### Manifest-driven addon discovery via init-data

The current design passes verity parameters for each addon on the kernel
command line. This works well for a small number of addons but does not
scale: each additional addon adds a long `kata.addon.<name>.verity_params=...`
entry, and the kernel command line has practical size limits.

A future evolution could introduce a **two-phase bootstrap**:

1. **Phase 1 (kernel-params driven)** — the kernel command line carries
   verity parameters for a single, small **manifest addon** image. This
   image is mounted first using the existing mechanism.

2. **Phase 2 (manifest driven)** — the manifest addon contains a
   configuration file (e.g. `addons.conf`) listing all other addons with
   their names, verity parameters, and any additional metadata. The mount
   script reads this file and mounts the remaining addons accordingly.

This approach has several advantages:

- **Kernel command line stays fixed-size** regardless of how many addons are
  composed.
- **Attestation is simplified** — the TEE firmware measures one manifest
  hash on the command line; the verifier only needs to validate that single
  hash. The chain of trust extends from the measured kernel command line to
  the verified manifest to the verified addons.
- **Richer metadata** — the manifest can carry structured information beyond
  verity parameters: addon ordering constraints, version requirements,
  policies, or init-data payloads.

The design of the systemd units and mount script can support both modes:
if per-addon verity parameters are present on the kernel command line, they
are used directly (current behavior); if a manifest addon is present, it
is consulted for the remaining addons. This makes the manifest mode an
additive, backward-compatible evolution.

### Addon versioning and attestation

Future work may add version metadata to addon images, enabling the
attestation flow to verify not just integrity (via dm-verity) but also that
the specific expected version of each component is present.

## References

- [Kata Containers architecture](../architecture)
- [dm-verity documentation](https://docs.kernel.org/admin-guide/device-mapper/verity.html)
- [EROFS filesystem](https://docs.kernel.org/filesystems/erofs.html)
- [systemd-sysext](https://www.freedesktop.org/software/systemd/man/latest/systemd-sysext.html)
- [bootc](https://github.com/containers/bootc)
- [ostree](https://ostreedev.github.io/ostree/)

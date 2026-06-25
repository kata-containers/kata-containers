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

### Agent-side: data-driven component manifest

> **Implementation note.** An earlier revision of this proposal described the
> agent as resolving a small, hardcoded list of component paths. While
> implementing and testing the addon we found this too rigid: the addon
> evolved to ship multiple attestation-agent flavours, per-process environment
> requirements, and ordering constraints that the agent should not have to know
> about. The design below — a **data-driven manifest** owned by the addon — is
> what we converged on so that adding or reconfiguring a bundled component
> requires **no** kata-agent code change.

Each addon ships a manifest at `etc/kata-addons/components.toml`. When the addon
is mounted, the kata-agent reads it and builds its launch plan from it. The
manifest declares the processes to launch and the resources they expose; all
paths are **relative to the addon mount root** (`/run/kata-addons/<name>`).

A process entry carries:

- `id` and a `level` used to order and gate launches.
- `args` (and `optional_args`, which are appended only when a named context
  variable is non-empty).
- `config`, `wait_socket` (the agent blocks until the socket appears), and
  `timeout_secs`.
- `env` — extra environment variables for the spawned process.
- An optional `select` selector plus `[process.variants.<name>]` tables, so a
  single logical component can ship multiple binaries and the consumer picks
  one at runtime.

`${...}` tokens in `args`, `config`, `env` values, and variant fields are
substituted by the agent from a runtime context it assembles (socket and config
paths, the addon mount root `${addon_root}`, the selected `${attester_variant}`,
etc.). Introducing a brand-new variable is the only change that ever needs to
touch the agent.

Abridged manifest (see "Attester variant selection" and "Runtime dependencies"
below for why the `nvidia` variant and the `PATH`/`LD_LIBRARY_PATH` entries are
shaped the way they are):

```toml
schema_version = 1

[paths]
"ocicrypt-config" = "etc/ocicrypt_config.json"
"pause-bundle"    = "pause_bundle"

[[process]]
id          = "attestation-agent"
level       = 1
args        = ["--attestation_sock", "${aa_attestation_uri}"]
config      = "${aa_config_path}"
wait_socket = "${aa_attestation_socket}"
select      = "${attester_variant}"

  [process.variants.default]
  path = "usr/local/bin/attestation-agent"

  [process.variants.nvidia]
  path = "usr/local/bin/attestation-agent-nv"
  env  = { LD_LIBRARY_PATH = "${addon_root}/usr/local/lib:/run/kata-addons/gpu/usr/lib" }

[[process]]
id          = "confidential-data-hub"
level       = 2
path        = "usr/local/bin/confidential-data-hub"
config      = "${cdh_config_path}"
env         = { OCICRYPT_KEYPROVIDER_CONFIG = "${ocicrypt_config_path}", PATH = "${addon_root}/usr/sbin:/bin:/sbin:/usr/bin:/usr/sbin" }
wait_socket = "${cdh_socket}"

[[process]]
id   = "api-server-rest"
level = 3
path = "usr/local/bin/api-server-rest"
args = ["--features", "${rest_api_features}"]
```

When **no** addon is mounted, the agent falls back to a built-in launch plan
that reproduces the legacy behaviour (components launched from
`/usr/local/bin/...` in the rootfs). The same dual-path principle applies to
the non-process resources declared under `[paths]`: the agent resolves them
inside the addon first and falls back to the legacy location otherwise:

| Resource               | Addon path                                              | Legacy path                     |
|------------------------|---------------------------------------------------------|---------------------------------|
| attestation-agent(-nv) | `/run/kata-addons/coco/usr/local/bin/attestation-agent[-nv]` | `/usr/local/bin/attestation-agent` |
| confidential-data-hub  | `/run/kata-addons/coco/usr/local/bin/confidential-data-hub` | `/usr/local/bin/confidential-data-hub` |
| api-server-rest        | `/run/kata-addons/coco/usr/local/bin/api-server-rest`   | `/usr/local/bin/api-server-rest` |
| ocicrypt_config.json   | `/run/kata-addons/coco/etc/ocicrypt_config.json`        | `/etc/ocicrypt_config.json`     |
| pause_bundle           | `/run/kata-addons/coco/pause_bundle`                    | `/pause_bundle`                 |

This approach:

- Preserves **backward compatibility** with existing monolithic rootfs images
  where CoCo components are baked into the base image.
- Requires **no special rootfs modifications** — the base image does not need
  stub files or directories for the addon components.
- Works transparently on a **read-only rootfs** — no bind-mounting, no
  remounting, no writes to the root filesystem.
- Keeps the agent **generic** — addon-specific names, env, and ordering live
  in the manifest, not in agent code.

### Attester variant selection and the NVRC contract

The CoCo addon ships **two** attestation-agent builds: the stock
`attestation-agent` and an NVIDIA-attester build, `attestation-agent-nv`, that
collects GPU evidence in addition to the TEE evidence. Which one runs is chosen
by the manifest's `select = "${attester_variant}"` selector, and the value of
`${attester_variant}` is driven by the guest init:

- On a plain confidential guest the kata-agent runs init itself and the variable
  defaults to `default` → the stock attester launches.
- On a GPU guest, **NVRC** (the NVIDIA runtime config that owns early boot)
  detects the GPU addon and exports `KATA_ATTESTER_VARIANT=nvidia` before
  exec'ing the kata-agent. The agent forwards that into the substitution context
  as `${attester_variant}`, so the `nvidia` variant launches.

This is a small **cross-component contract**: the environment variable name
(`KATA_ATTESTER_VARIANT`) and the `nvidia` value are produced by NVRC and
consumed by the kata-agent, which keeps the agent free of any GPU- or
NVIDIA-specific knowledge — it only knows how to forward a selector into the
manifest. We arrived at this split after first trying to special-case the
attester inside the agent; pushing the decision out to NVRC + the manifest kept
both the agent and the addon generic.

#### Why one addon with two builds (and not two addons)

Shipping **two** attestation-agent builds inside a **single** CoCo addon is a
deliberate choice, and it is worth being explicit about it because addons are
otherwise meant to *eliminate* duplication.

- A single CoCo addon serves **both** plain confidential guests (TEE evidence
  only) and confidential GPU guests (TEE + GPU evidence). The only thing that
  differs between them is which attestation-agent binary runs; everything else
  in the addon (confidential-data-hub, api-server-rest, ocicrypt config, pause
  bundle, `cryptsetup`) is shared verbatim. Splitting the NVIDIA attester into
  its own addon would duplicate that shared payload and force every confidential
  GPU guest to compose two CoCo-flavoured addons instead of one.
- The **cost** of keeping both builds together is precisely the manifest's
  `select`/`variants` machinery: the manifest has to be aware that the
  attestation-agent comes in two flavours and pick one at runtime. We consider
  that a fair trade — the complexity is confined to data (the manifest), the
  agent stays generic, and the addon stays a single, coherent "CoCo" unit.
- A separate addon only pays off when its contents are **substantially**
  different (e.g. the GPU addon, which carries the driver userspace), not for
  two near-identical builds of the same component. No further attester variants
  are planned today, but if one appeared it would be another `[process.variants.<name>]`
  entry — not a new image.

### Runtime dependencies: dynamic linking and secure-mount tooling

Two classes of runtime dependency surfaced only once the components actually ran
inside a composed VM. Both are resolved by the manifest `env` entries above
rather than by agent code, and both informed where binaries and libraries must
physically live.

#### NVIDIA attester dynamic libraries

`attestation-agent-nv` links `libnvat.so` (the NVIDIA Attestation SDK), which in
turn:

- `dlopen`s `libnvidia-ml.so.1` (NVML) at runtime to gather GPU evidence. NVML
  is part of the **GPU addon**, mounted at `/run/kata-addons/gpu` with its driver
  libraries under `usr/lib`.
- pulls in a closure of non-glibc libraries (libxml2, zlib, lzma, the C++
  runtime, ...) that the guest rootfs does not otherwise ship.

Neither set is present in a stock guest, so:

- The CoCo addon build bundles `libnvat.so` **and every non-glibc transitive
  dependency** next to it under `usr/local/lib`.
- The `nvidia` **manifest variant** (the `[process.variants.nvidia]` entry, not
  a separate image) sets an `LD_LIBRARY_PATH` that lists **both** the CoCo
  addon's `usr/local/lib` (for `libnvat` and its closure) **and** the GPU
  addon's `usr/lib` (for NVML). Setting only the first was the cause of an
  `NVAT Error 500: NVML Initialization Failed` we hit during bring-up; the
  RCAR handshake then never produced GPU evidence and attestation failed.

Because the agent applies manifest `env` on top of the inherited environment
(without clearing it), and because the `nvidia` variant only ever runs when the
GPU addon is present, referencing the GPU addon's well-known mount path here is
safe.

#### CDH secure-mount tooling (encrypted vs plain storage)

The confidential-data-hub `secure_mount` feature shells out — by `PATH`
lookup — to external tools, and these split cleanly into two buckets that belong
in two different places:

- **Plain storage setup** — `mke2fs`/`mkfs.ext4` (and `dd`, plus
  `/etc/mke2fs.conf`) format the scratch volume. This is needed for *unencrypted*
  ephemeral storage too, so it belongs in the **base image** and ships there
  unconditionally.
- **Encrypted storage** — `cryptsetup` LUKS-formats and opens the volume. This is
  a CoCo-only capability, so it belongs with the CoCo guest components in the
  **coco addon**.

This is the same `veritysetup`-vs-`cryptsetup` reasoning already applied to addon
mounting: the base must always carry `veritysetup` (it opens *every* addon as a
dm-verity device before mounting), and `cryptsetup` shares an identical
shared-library closure, so wherever `veritysetup` lives the libraries for
`cryptsetup` are already present.

Because CDH runs in the **base rootfs namespace** but `cryptsetup` lives in the
addon (which is not on the default search path), the manifest sets CDH's `PATH`
to `${addon_root}/usr/sbin:/bin:/sbin:/usr/bin:/usr/sbin` — the addon's
`cryptsetup` first, then the base directories that carry `mke2fs`/`mkfs.ext4`/`dd`.
(The kata-agent launches components with `PATH=/bin:/sbin:/usr/bin:/usr/sbin`;
since setting any `env` value replaces that variable wholesale, the base
directories are restored explicitly.)

How each tool is provisioned depends on the **base flavour** (see
"Base image flavours" below), but the placement *contract* is identical:

| Tool                         | Bucket             | Full-distro base (Ubuntu)                                | Distroless base (chiseled NVIDIA + NVRC)                |
|------------------------------|--------------------|----------------------------------------------------------|---------------------------------------------------------|
| `veritysetup`                | base, always       | `cryptsetup-bin` (unconditional in `ubuntu/config.sh`)   | copied into the base layout unconditionally             |
| `cryptsetup`                 | coco addon         | also present in the full-distro base via `cryptsetup-bin`| binary bundled in the coco addon; libs come from base   |
| `mke2fs`/`mkfs.ext4`/`dd`    | base, for CoCo     | `e2fsprogs` (on `CONFIDENTIAL_GUEST=yes`) + `coreutils`  | copied into the base layout                             |

The distroless path needs explicit copying because nothing lands in a chiseled
image unless placed there deliberately, and the NVIDIA base is never built with
`CONFIDENTIAL_GUEST=yes`. The full-distro base is built with
`CONFIDENTIAL_GUEST=yes`, so the same tools arrive as ordinary packages. In both
cases the addon's `cryptsetup` resolves its libraries against the base, which
requires the base and the coco-addon builder to stay on the **same distro/ABI**
(Ubuntu 24.04 "noble" today).

### Image build pipeline

#### Base image flavours

Kata base images come in two flavours, distinguished by **who owns early boot**.
This distinction — not "Ubuntu vs NVIDIA" — is what drives the differences in
how tooling is provisioned and who mounts the addons:

- **Full-distro base** — ships a complete distribution with **systemd** as init.
  systemd discovers and mounts the addons (via `kata-addon-mount@.service`), and
  the binaries/libraries the guest needs arrive as ordinary distribution
  packages. The standard confidential `kata-containers.img` (Ubuntu) is today's
  instance.
- **Distroless base** — a minimal, chiseled image with no full init system. A
  dedicated early-boot component takes over the responsibilities systemd would
  otherwise have (addon discovery and mounting, attester selection,
  orchestration), and any tooling must be placed into the image deliberately
  rather than pulled in as packages. The chiseled `base-nvidia` driven by
  **NVRC** is today's instance.

These are the only two flavours today and no others are planned, but a new base
would fall into one of these categories and follow the same mechanisms (systemd
units for a full-distro base; an NVRC-like early-boot owner for a distroless
one). The sections below describe the build for both; where they diverge it is
because of the flavour, not because the NVIDIA image is a special case.

#### Base image

The full-distro `kata-containers.img` is built as before, but **without** CoCo
guest components (attestation-agent, confidential-data-hub, api-server-rest,
ocicrypt config, pause bundle). It includes:

- The kata-agent
- systemd and the `kata-addon-mount@.service` template unit
- `cryptsetup-bin` (provides `veritysetup`) — required unconditionally so
  that the base image can mount verity-protected addons regardless of whether
  the base itself was built with `CONFIDENTIAL_GUEST=yes`. On Ubuntu this same
  package also provides `cryptsetup`, so the full-distro base happens to carry
  the encrypted-storage binary too.
- The plain-storage tooling for CDH `secure_mount` — `mke2fs`/`mkfs.ext4`,
  `dd`, and `/etc/mke2fs.conf`. On the standard base these come from
  `e2fsprogs` (added when `CONFIDENTIAL_GUEST=yes`) and `coreutils`. See
  "Runtime dependencies" for why the encrypted-storage `cryptsetup` lives in
  the addon instead.

The **distroless base** (`base-nvidia`, driven by NVRC) is a chiseled,
driver-agnostic image rather than a full distro, so the items above do not
arrive as packages — they are copied into the base layout explicitly:
`veritysetup` and its library closure unconditionally, and the
`mke2fs`/`mkfs.ext4`/`dd`/`mke2fs.conf` plain-storage tooling alongside it.

#### CoCo addon image

The `kata-containers-coco-addon.img` is built by:

1. Unpacking the CoCo guest components tarball into a temporary rootfs. Besides
   the agent-launched binaries (attestation-agent, attestation-agent-nv,
   confidential-data-hub, api-server-rest) this tarball also carries:
   - `cryptsetup` under `usr/sbin` — the encrypted-storage binary for CDH
     `secure_mount` (its shared libraries are resolved against the base; see
     "Runtime dependencies").
   - the NVIDIA attester libraries under `usr/local/lib` — `libnvat.so` plus its
     non-glibc transitive closure (libxml2, zlib, lzma, the C++ runtime).
2. Unpacking the pause image tarball into the same rootfs.
3. Writing the component manifest to `etc/kata-addons/components.toml`.
4. Running the image builder with:
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

The architecture is designed to support multiple addon images:

- **GPU addon** — the NVIDIA GPU userspace (driver libraries, NVML, the
  container-toolkit binaries, kernel modules) lives in a `gpu-addon` image
  mounted at `/run/kata-addons/gpu`, carved out of the same build as the
  driver-agnostic `base-nvidia` image. NVRC orchestrates early boot, loads the
  modules from the addon, and composes the GPU addon with the CoCo addon on
  confidential GPU guests. This addon is implemented; its interplay with the
  CoCo addon (NVML resolution, attester selection) is covered in
  "Runtime dependencies" and "Attester variant selection" above.

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

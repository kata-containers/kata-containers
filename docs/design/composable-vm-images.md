# Composable VM Images for Kata Containers

> **Status**: Implemented

## Summary

This proposal introduces a **composable image architecture** for Kata Containers
guest VMs. Instead of building a single monolithic rootfs image that contains
every component a workload might need, the runtime assembles a VM from a lean
**base image** plus zero or more purpose-specific **guest extension images** that are
cold-plugged as additional virtio-blk devices. Each extension is an EROFS image
protected by dm-verity, mounted read-only inside the guest before the kata-agent
starts.

The first application of this architecture splits **Confidential Containers
(CoCo) guest components** out of the monolithic `kata-containers-confidential.img`
into a separate `kata-containers-coco-extension.img`.

The current proposal targets **QEMU** as the hypervisor backend. The
design is intentionally hypervisor-agnostic at the configuration and guest
layers — the `guest_extension_images` configuration, guest-side systemd units, and
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
generic, guest extension images are independently built and versioned, and the runtime
composes them at VM creation time.

## Design

### Architecture overview

```
Host                                          Guest VM
┌───────────────────────────────┐
│  Runtime config               │
│  ┌─────────────────────────┐  │
│  │ image = base            │  │      ┌─────────────────────────────────────┐
│  │ guest_extension_images: │──┼─────>│  /  (base rootfs, erofs)            │
│  │   - name: coco          │  │ boot │    kata-agent, systemd, ...         │
│  │     path: ...           │  │ cold │                                     │
│  │     verity: ...         │  │ plug │  /run/kata-extensions/coco/ (erofs) │
│  └─────────────────────────┘  │      │    attestation-agent                │
│                               │      │    confidential-data-hub            │
│  QEMU                         │      │    api-server-rest                  │
│    -drive base.img            │      │    ocicrypt_config.json             │
│    -drive coco.img            │      │    pause_bundle/                    │
│    serial=extension-coco      │      │                                     │
└───────────────────────────────┘      │  dm-verity protects both the        │
                                       │  base rootfs and the extension.     │
                                       └─────────────────────────────────────┘
```

### Runtime configuration

A new `guest_extension_images` field on the hypervisor configuration accepts an ordered
list of guest extension images:

```toml
[hypervisor.qemu]
image = "/opt/kata/share/kata-containers/kata-containers.img"

# ...existing keys...

[[hypervisor.qemu.guest_extension_images]]
name = "coco"
path = "/opt/kata/share/kata-containers/kata-containers-coco-extension.img"
verity_params = "root_hash=abc...,salt=def...,data_blocks=1234,hash_block_size=4096,data_block_size=4096"
```

Each entry maps to a Rust struct:

```rust
pub struct GuestExtensionImage {
    pub name: String,
    pub path: String,
    pub verity_params: String,
}
```

The `name` field is the primary identifier for an extension. It must be unique
within the configuration and is used to:

- Set the **virtio-blk serial** to `extension-<name>`, enabling deterministic
  device discovery in the guest via `/sys/block/*/serial`.
- Namespace the **kernel command-line** verity parameters as
  `kata.extension.<name>.verity_params=...`.
- Name the guest **mount point** at `/run/kata-extensions/<name>/`.
- Name the **dm-verity device mapper target** as `extension-<name>`.

### Host-side: cold-plugging extension devices

At VM creation time, for each entry in `guest_extension_images`, the runtime:

1. Creates a virtio-blk device backed by the guest extension image file.
   Extensions are **always** attached as virtio-blk, using the architecture's
   virtio-blk transport (`virtio-blk-ccw` on s390x, `virtio-blk-pci`
   elsewhere). Neither the VM rootfs driver (`vm_rootfs_driver`) nor the
   generic block device driver (`block_device_driver`) is reused here: those
   may resolve to a non-virtio-blk transport such as `virtio-pmem` (NVDIMM) or
   `virtio-scsi`, and only virtio-blk devices carry the serial the guest relies
   on for discovery (step 2), so a non-virtio-blk transport would leave the
   extension undiscoverable and its mount unit would fail closed.
2. Sets the device serial to `extension-<name>`.
3. Appends `kata.extension.<name>.verity_params=<value>` to the guest kernel
   command line for *every* configured extension. The value is empty for an
   unmeasured extension (`verity_params = ""`, e.g. on s390x — see "Mount
   script" below); the entry is emitted unconditionally because it doubles as
   the guest-side activation signal for the mount unit. With an empty value the
   entry renders as a bare `kata.extension.<name>.verity_params` (no `=`), which
   the unit condition and generator both handle.

Both the Go runtime and the Rust runtime-rs implement this logic in their
respective QEMU hypervisor backends. The extension devices are **cold-plugged** —
they are present on the QEMU command line at VM start, not hot-plugged later.

Other hypervisor backends (Cloud Hypervisor, Firecracker) do not implement
extension cold-plugging yet, but the mechanism is straightforward to add: each
backend only needs to translate the `GuestExtensionImage` entries into its native
block device attachment API. The runtime configuration, guest-side mounting,
and agent path resolution are completely hypervisor-independent.

### Guest-side: mounting extensions

A systemd template unit `kata-extension-mount@.service` handles extension discovery and
mounting. The unit is instantiated per extension name (e.g.
`kata-extension-mount@coco.service`); a systemd generator (described below)
creates those instances automatically from the kernel command line.

The mounting is performed by a systemd unit (before the agent starts) rather
than by the kata-agent itself for several reasons:

- **Chicken-and-egg problem** — in the CoCo extension case, the extension carries
  the very guest components (attestation-agent, confidential-data-hub) that
  the kata-agent needs to launch. The agent cannot mount the extension because
  it needs the extension's contents to be available before it starts.

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
  entity manages early boot. The key requirement is that guest extension images are
  mounted before the kata-agent starts — how that is achieved is an
  init-system concern.

#### Service unit

```ini
[Unit]
Description=Mount Kata guest extension image %i
DefaultDependencies=no
Before=kata-agent.service
After=local-fs-pre.target
ConditionKernelCommandLine=kata.extension.%i.verity_params
OnFailure=poweroff.target

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/libexec/kata-extension-mount.sh %i
ExecStop=/usr/libexec/kata-extension-umount.sh %i

[Install]
WantedBy=kata-containers.target
```

Key design decisions:

- **`ConditionKernelCommandLine`** — the service only activates when the
  runtime has actually configured the extension. This prevents the unit from
  running (and failing) in non-extension VM configurations.

- **`Before=kata-agent.service`** — guarantees the extension filesystem is mounted
  before the agent attempts to use any component from it.

- **`OnFailure=poweroff.target`** — if the extension cannot be mounted (e.g.
  verity verification failure, missing device), the VM is shut down
  immediately. A confidential VM must not continue with an unverified or
  missing extension.

#### Enabling instances

> **Implementation note.** An earlier revision enabled a fixed
> `kata-extension-mount@coco.service` symlink at rootfs build time, which would
> have required editing the image build for every new extension. Review feedback
> pushed us to the generator below so the build stays extension-agnostic.

systemd template units must be enabled per instance, and the set of extensions
is only known at runtime (from the kernel command line). A systemd generator,
`kata-extension-mount-generator`, bridges that gap: it runs in early boot, scans
`/proc/cmdline` for every `kata.extension.<name>.verity_params` entry (whether it
carries a value or, for an unmeasured extension, is a bare key), and symlinks
`kata-extension-mount@<name>.service` into `kata-containers.target.wants`.

Because the runtime emits exactly one such cmdline entry per configured
`guest_extension_images`, the generator enables precisely the extensions the VM
was configured with. Adding a new extension therefore requires no change to the
rootfs build — it is wired up entirely from runtime configuration. The generator
ships in the base image (installed alongside the template unit by the agent), so
it is part of the dm-verity-measured rootfs, and the cmdline it reads is itself
covered by the guest launch measurement.

#### Mount script

The mount script (`kata-extension-mount.sh`) performs the following steps:

1. **Device discovery** — scans `/sys/block/*/serial` for a device whose
   serial matches `extension-<name>`. This is more reliable than waiting for udev
   to create `/dev/disk/by-id/` symlinks, since the minimal guest environment
   may not run a full udev daemon.

2. **Verity parameter extraction** — reads
   `kata.extension.<name>.verity_params=...` from `/proc/cmdline` and parses the
   comma-separated key=value pairs (root_hash, salt, data_blocks,
   hash_block_size, data_block_size).

3. **dm-verity setup** — runs `veritysetup open` with `--no-superblock` to
   open the verity target. The `--no-superblock` flag is required because the
   image builder uses `veritysetup format --no-superblock` during image
   creation — all verity parameters are passed explicitly rather than stored
   in an on-disk superblock.

4. **EROFS mount** — mounts the resulting device read-only at
   `/run/kata-extensions/<name>/` (the dm-verity target when verified, or the
   data partition directly for an unmeasured extension).

##### Integrity policy: measured vs. unmeasured, and failing closed

An extension can legitimately ship *without* dm-verity — for example on s390x,
where IBM Secure Execution protects the guest through a different mechanism and
images are built with `MEASURED_ROOTFS=no`. The mount script must therefore
support a raw (unverified) mount, but it must **not** let that path become a
silent downgrade: an attacker who can edit the (host-supplied) kernel command
line could otherwise strip `verity_params` from a *measured* extension and have
it mounted unverified.

The script separates these two cases explicitly, using the **on-disk layout as
the source of truth** rather than trusting the cmdline alone. The image build
encodes its integrity policy in the partition table (see "Image build" below):
a measured extension carries a dm-verity **hash partition** (`p2`) next to the
data partition (`p1`), while an unmeasured extension has only `p1`. The script
detects the presence of the hash device and cross-checks it against the
`verity_params` carried on the kernel command line (which, in a confidential
guest, is itself part of the measured, attested boot):

| Hash device (`p2`) | `verity_params` on cmdline | Action                                             |
|--------------------|----------------------------|----------------------------------------------------|
| present            | present                    | **verify** with dm-verity, then mount (normal case)|
| present            | absent / empty             | **refuse** — verity was stripped/disabled (tamper) |
| absent             | present                    | **refuse** — params but nothing to verify (mismatch)|
| absent             | absent / empty             | **raw mount** — genuinely unmeasured extension     |

Any "refuse" path exits non-zero; combined with `OnFailure=poweroff.target` on
the mount unit, that powers the VM off rather than continuing with an unverified
or inconsistent extension. The defence has two layers: the cmdline (and thus the
root hash, or its deliberate absence) is covered by the guest launch
measurement and remote attestation, and the in-guest layout cross-check fails
closed so a measured extension can never be silently downgraded to a raw mount.

### Agent-side: data-driven component manifest

> **Implementation note.** An earlier revision of this proposal described the
> agent as resolving a small, hardcoded list of component paths. While
> implementing and testing the extension we found this too rigid: the extension
> evolved to ship multiple attestation-agent flavours, per-process environment
> requirements, and ordering constraints that the agent should not have to know
> about. The design below — a **data-driven manifest** owned by the extension — is
> what we converged on so that adding or reconfiguring a bundled component
> requires **no** kata-agent code change.

Each extension ships a manifest at `etc/kata-extensions/components.toml`. When the extension
is mounted, the kata-agent reads it and builds its launch plan from it. The
manifest declares the processes to launch and the resources they expose; all
paths are **relative to the extension mount root** (`/run/kata-extensions/<name>`).

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
paths, the extension mount root `${extension_root}`, the selected `${attester_variant}`,
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
  env  = { LD_LIBRARY_PATH = "${extension_root}/usr/local/lib:/run/kata-extensions/gpu/usr/lib" }

[[process]]
id          = "confidential-data-hub"
level       = 2
path        = "usr/local/bin/confidential-data-hub"
config      = "${cdh_config_path}"
env         = { OCICRYPT_KEYPROVIDER_CONFIG = "${ocicrypt_config_path}", PATH = "${extension_root}/usr/sbin:/bin:/sbin:/usr/bin:/usr/sbin" }
wait_socket = "${cdh_socket}"

[[process]]
id   = "api-server-rest"
level = 3
path = "usr/local/bin/api-server-rest"
args = ["--features", "${rest_api_features}"]
```

When **no** extension is mounted, the agent falls back to a built-in launch plan
that reproduces the legacy behaviour (components launched from
`/usr/local/bin/...` in the rootfs). The same dual-path principle applies to
the non-process resources declared under `[paths]`: the agent resolves them
inside the extension first and falls back to the legacy location otherwise:

| Resource               | Extension path                                              | Legacy path                     |
|------------------------|---------------------------------------------------------|---------------------------------|
| attestation-agent(-nv) | `/run/kata-extensions/coco/usr/local/bin/attestation-agent[-nv]` | `/usr/local/bin/attestation-agent` |
| confidential-data-hub  | `/run/kata-extensions/coco/usr/local/bin/confidential-data-hub` | `/usr/local/bin/confidential-data-hub` |
| api-server-rest        | `/run/kata-extensions/coco/usr/local/bin/api-server-rest`   | `/usr/local/bin/api-server-rest` |
| ocicrypt_config.json   | `/run/kata-extensions/coco/etc/ocicrypt_config.json`        | `/etc/ocicrypt_config.json`     |
| pause_bundle           | `/run/kata-extensions/coco/pause_bundle`                    | `/pause_bundle`                 |

This approach:

- Preserves **backward compatibility** with existing monolithic rootfs images
  where CoCo components are baked into the base image.
- Requires **no special rootfs modifications** — the base image does not need
  stub files or directories for the extension components.
- Works transparently on a **read-only rootfs** — no bind-mounting, no
  remounting, no writes to the root filesystem.
- Keeps the agent **generic** — extension-specific names, env, and ordering live
  in the manifest, not in agent code.

### Attester variant selection and the NVRC contract

The CoCo extension ships **two** attestation-agent builds: the stock
`attestation-agent` and an NVIDIA-attester build, `attestation-agent-nv`, that
collects GPU evidence in addition to the TEE evidence. Which one runs is chosen
by the manifest's `select = "${attester_variant}"` selector, and the value of
`${attester_variant}` is driven by the guest init:

- On a plain confidential guest the kata-agent runs init itself and the variable
  defaults to `default` → the stock attester launches.
- On a GPU guest, **NVRC** (the NVIDIA runtime config that owns early boot)
  detects the GPU extension and exports `KATA_ATTESTER_VARIANT=nvidia` before
  exec'ing the kata-agent. The agent forwards that into the substitution context
  as `${attester_variant}`, so the `nvidia` variant launches.

This is a small **cross-component contract**: the environment variable name
(`KATA_ATTESTER_VARIANT`) and the `nvidia` value are produced by NVRC and
consumed by the kata-agent, which keeps the agent free of any GPU- or
NVIDIA-specific knowledge — it only knows how to forward a selector into the
manifest. We arrived at this split after first trying to special-case the
attester inside the agent; pushing the decision out to NVRC + the manifest kept
both the agent and the extension generic.

#### Why one extension with two builds (and not two extensions)

Shipping **two** attestation-agent builds inside a **single** CoCo extension is a
deliberate choice, and it is worth being explicit about it because extensions are
otherwise meant to *eliminate* duplication.

- A single CoCo extension serves **both** plain confidential guests (TEE evidence
  only) and confidential GPU guests (TEE + GPU evidence). The only thing that
  differs between them is which attestation-agent binary runs; everything else
  in the extension (confidential-data-hub, api-server-rest, ocicrypt config, pause
  bundle, `cryptsetup`) is shared verbatim. Splitting the NVIDIA attester into
  its own extension would duplicate that shared payload and force every confidential
  GPU guest to compose two CoCo-flavoured extensions instead of one.
- The **cost** of keeping both builds together is precisely the manifest's
  `select`/`variants` machinery: the manifest has to be aware that the
  attestation-agent comes in two flavours and pick one at runtime. We consider
  that a fair trade — the complexity is confined to data (the manifest), the
  agent stays generic, and the extension stays a single, coherent "CoCo" unit.
- A separate extension only pays off when its contents are **substantially**
  different (e.g. the GPU extension, which carries the driver userspace), not for
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
  is part of the **GPU extension**, mounted at `/run/kata-extensions/gpu` with its driver
  libraries under `usr/lib`.
- pulls in a closure of non-glibc libraries (libxml2, zlib, lzma, the C++
  runtime, ...) that the guest rootfs does not otherwise ship.

Neither set is present in a stock guest, so:

- The CoCo extension build bundles `libnvat.so` **and every non-glibc transitive
  dependency** next to it under `usr/local/lib`.
- The `nvidia` **manifest variant** (the `[process.variants.nvidia]` entry, not
  a separate image) sets an `LD_LIBRARY_PATH` that lists **both** the CoCo
  extension's `usr/local/lib` (for `libnvat` and its closure) **and** the GPU
  extension's `usr/lib` (for NVML). Setting only the first was the cause of an
  `NVAT Error 500: NVML Initialization Failed` we hit during bring-up; the
  RCAR handshake then never produced GPU evidence and attestation failed.

Because the agent applies manifest `env` on top of the inherited environment
(without clearing it), and because the `nvidia` variant only ever runs when the
GPU extension is present, referencing the GPU extension's well-known mount path here is
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
  **coco extension**.

This is the same `veritysetup`-vs-`cryptsetup` reasoning already applied to extension
mounting: the base must always carry `veritysetup` (it opens *every* extension as a
dm-verity device before mounting), and `cryptsetup` shares an identical
shared-library closure, so wherever `veritysetup` lives the libraries for
`cryptsetup` are already present.

Because CDH runs in the **base rootfs namespace** but `cryptsetup` lives in the
extension (which is not on the default search path), the manifest sets CDH's `PATH`
to `${extension_root}/usr/sbin:/bin:/sbin:/usr/bin:/usr/sbin` — the extension's
`cryptsetup` first, then the base directories that carry `mke2fs`/`mkfs.ext4`/`dd`.
(The kata-agent launches components with `PATH=/bin:/sbin:/usr/bin:/usr/sbin`;
since setting any `env` value replaces that variable wholesale, the base
directories are restored explicitly.)

How each tool is provisioned depends on the **base flavour** (see
"Base image flavours" below), but the placement *contract* is identical:

| Tool                         | Bucket             | Full-distro base (Ubuntu)                                | Distroless base (chiseled NVIDIA + NVRC)                |
|------------------------------|--------------------|----------------------------------------------------------|---------------------------------------------------------|
| `veritysetup`                | base, always       | `cryptsetup-bin` (unconditional in `ubuntu/config.sh`)   | copied into the base layout unconditionally             |
| `cryptsetup`                 | coco extension         | also present in the full-distro base via `cryptsetup-bin`| binary bundled in the coco extension; libs come from base   |
| `mke2fs`/`mkfs.ext4`/`dd`    | base, for CoCo     | `e2fsprogs` (on `CONFIDENTIAL_GUEST=yes`) + `coreutils`  | copied into the base layout                             |

The distroless path needs explicit copying because nothing lands in a chiseled
image unless placed there deliberately, and the NVIDIA base is never built with
`CONFIDENTIAL_GUEST=yes`. The full-distro base is built with
`CONFIDENTIAL_GUEST=yes`, so the same tools arrive as ordinary packages. In both
cases the extension's `cryptsetup` resolves its libraries against the base, which
requires the base and the coco-extension builder to stay on the **same distro/ABI**
(Ubuntu 24.04 "noble" today).

### Image build pipeline

#### Base image flavours

Kata base images come in two flavours, distinguished by **who owns early boot**.
This distinction — not "Ubuntu vs NVIDIA" — is what drives the differences in
how tooling is provisioned and who mounts the extensions:

- **Full-distro base** — ships a complete distribution with **systemd** as init.
  systemd discovers and mounts the extensions (via `kata-extension-mount@.service`), and
  the binaries/libraries the guest needs arrive as ordinary distribution
  packages. The standard confidential `kata-containers.img` (Ubuntu) is today's
  instance.
- **Distroless base** — a minimal, chiseled image with no full init system. A
  dedicated early-boot component takes over the responsibilities systemd would
  otherwise have (extension discovery and mounting, attester selection,
  orchestration), and any tooling must be placed into the image deliberately
  rather than pulled in as packages. The chiseled `nvidia` base driven by
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
- systemd and the `kata-extension-mount@.service` template unit
- `cryptsetup-bin` (provides `veritysetup`) — required unconditionally so
  that the base image can mount verity-protected extensions regardless of whether
  the base itself was built with `CONFIDENTIAL_GUEST=yes`. On Ubuntu this same
  package also provides `cryptsetup`, so the full-distro base happens to carry
  the encrypted-storage binary too.
- The plain-storage tooling for CDH `secure_mount` — `mke2fs`/`mkfs.ext4`,
  `dd`, and `/etc/mke2fs.conf`. On the standard base these come from
  `e2fsprogs` (added when `CONFIDENTIAL_GUEST=yes`) and `coreutils`. See
  "Runtime dependencies" for why the encrypted-storage `cryptsetup` lives in
  the extension instead.

The **distroless base** (`nvidia`, driven by NVRC) is a chiseled,
driver-agnostic image rather than a full distro, so the items above do not
arrive as packages — they are copied into the base layout explicitly:
`veritysetup` and its library closure unconditionally, and the
`mke2fs`/`mkfs.ext4`/`dd`/`mke2fs.conf` plain-storage tooling alongside it.

#### CoCo guest extension image

The `kata-containers-coco-extension.img` is built by:

1. Unpacking the CoCo guest components tarball into a temporary rootfs. Besides
   the agent-launched binaries (attestation-agent, attestation-agent-nv,
   confidential-data-hub, api-server-rest) this tarball also carries:
   - `cryptsetup` under `usr/sbin` — the encrypted-storage binary for CDH
     `secure_mount` (its shared libraries are resolved against the base; see
     "Runtime dependencies").
   - the NVIDIA attester libraries under `usr/local/lib` — `libnvat.so` plus its
     non-glibc transitive closure (libxml2, zlib, lzma, the C++ runtime).
2. Unpacking the pause image tarball into the same rootfs.
3. Writing the component manifest to `etc/kata-extensions/components.toml`.
4. Running the image builder with:
   - `FS_TYPE=erofs` — EROFS filesystem for compact, read-only storage.
   - `MEASURED_ROOTFS=yes` — creates a dm-verity hash partition.
   - `SKIP_DAX_HEADER=yes` — no DAX header (virtio-blk, not NVDIMM).
   - `SKIP_ROOTFS_CHECK=yes` — the extension has no `/sbin/init`.
   - `BUILD_VARIANT=coco-extension` — produces a correctly named root hash file
     (`root_hash_coco-extension.txt`).

The resulting image is a two-partition disk:

- **Partition 1**: EROFS data partition containing the CoCo components.
- **Partition 2**: dm-verity hash partition.

The root hash and verity parameters are captured at build time and injected
into the runtime configuration templates.

### Security model

The extension architecture preserves the existing security guarantees of
Confidential Containers:

- **dm-verity** provides integrity protection for the guest extension image, identical
  to the existing protection on the base rootfs. Any tampering with the extension
  contents is detected at mount time.

- **Verity parameters on the kernel command line** are measured by the TEE
  firmware (OVMF/TDVF for TDX, SEV-SNP firmware) as part of the launch
  measurement. An attacker cannot substitute different verity parameters
  without changing the measurement, which would be detected during
  attestation.

- **Mount failure = VM shutdown** — `OnFailure=poweroff.target` ensures the
  VM does not proceed with missing or tampered components.

- **Read-only mounts** — both the base rootfs and guest extension images are mounted
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
  virtio-blk extension device, verify it with dm-verity, and place or symlink it
  where sysext expects to find it — eliminating much of the simplification
  sysext would offer.

- **Boot ordering** — `systemd-sysext.service` merges extensions before
  `basic.target`, but the precise ordering relative to `kata-agent.service`
  is not directly controllable. The current `Before=kata-agent.service`
  guarantee on the custom mount unit gives explicit control over this.

- **Version matching** — sysext enforces `extension-release.d` metadata
  matching against the host `os-release`. While this is useful for general
  purpose systems, it adds friction in the Kata context where the base image
  and guest extension images are built and versioned together in a controlled pipeline.

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
managed using bootc/ostree, and guest extension images would still be cold-plugged
and mounted at VM boot using the mechanism described here. bootc does not
provide a mechanism for dynamically composing additional block devices into
a running or booting system — it operates at the image build and deployment
layer, not at the VM assembly layer.

## Future work

### Additional extension types

The architecture is designed to support multiple guest extension images:

- **GPU extension** — the NVIDIA GPU userspace (driver libraries, NVML, the
  container-toolkit binaries, kernel modules) lives in a `gpu-extension` image
  mounted at `/run/kata-extensions/gpu`, carved out of the same build as the
  driver-agnostic `nvidia` base image. NVRC orchestrates early boot, loads the
  modules from the extension, and composes the GPU extension with the CoCo extension on
  confidential GPU guests. This extension is implemented; its interplay with the
  CoCo extension (NVML resolution, attester selection) is covered in
  "Runtime dependencies" and "Attester variant selection" above.

- **NVIDIA devkit extension** — a self-contained minimal **Alpine** rootfs
  (musl + busybox + apk) with common debug tools prebaked (strace, gdb, ltrace,
  iproute2, procps, lsof, tcpdump, ...), mounted at `/run/kata-extensions/devkit`.
  Built standalone via `rootfs-image-nvidia-devkit-extension-tarball` (no driver
  stage-one, no chisel, no kata-agent): the build fetches the Alpine minirootfs
  and `apk add`s the toolset. At runtime the `devkit-enter` debug shell and the
  `devkit-apk` wrapper overlay a writable tmpfs on the read-only extension and
  chroot in, so apk and the tools run natively; `devkit-apk add <pkg>` works
  inside that chroot. Alpine is chosen so its musl sonames never collide with the
  base image's glibc (mounting the extension cannot shadow the base kata-agent)
  and to avoid dpkg maintainer-script fragility. Also produces
  `root_hash_nvidia-devkit-extension.txt` for `verity_params`; enabled through
  runtime-rs **drop-ins** (see `src/runtime-rs/config/drop-in-examples/`).
  Development use only — not for attested production/TEE deployments.

- **Custom extensions** — users can build their own guest extension images for
  workload-specific libraries, models, or configurations.

### Additive image assembly

Today the NVIDIA images are carved out of a single chiseled monolith tree: the
`gpu-extension` is assembled additively (an allow-list of GPU userspace is copied
into a fresh tree), while the `nvidia` base is produced *subtractively* — the same
allow-list is deleted from the full tree. This keeps the monolith byte-identical
during the transition, but it means the base build describes what it does *not*
want rather than what it does.

The intended next step is to invert this into a purely additive flow, so nothing
is ever subtracted:

1. Build the shared driver `stage-one` (always required).
2. Assemble the `nvidia` base additively from `stage-one`.
3. Assemble the `gpu-extension` additively from `stage-one`.
4. Compose the monolith by combining the base and the extension.

This requires splitting the interleaved `chisseled_*` producers (which today copy
base runtime libraries and GPU userspace in the same pass) into base and GPU
halves. It is best done together with the removal of the Go-runtime monolith, so
that only a single, additive code path remains rather than maintaining both the
monolith and the split builds.

### Extension ordering

When multiple extensions are configured, they are cold-plugged in the order they
appear in the `guest_extension_images` list. The current design does not enforce
explicit ordering dependencies between extensions. If future use cases require
extension-to-extension dependencies, the systemd units can be extended with
appropriate `After=`/`Requires=` relationships.

### Other hypervisor backends

The current proposal covers QEMU only. Extending to other backends
requires implementing block device cold-plug for each:

- **Cloud Hypervisor** — add `--disk` entries with the guest extension image path and
  serial. Cloud Hypervisor natively supports virtio-blk serial numbers.
- **Dragonball** — attach additional virtio-blk devices through the
  Dragonball VMM's block device configuration, mapping each `GuestExtensionImage`
  to a drive with the corresponding serial.
- **Firecracker** — add block device entries via the Firecracker API with
  the appropriate drive ID. Serial-based discovery may need adaptation since
  Firecracker exposes drive IDs differently.

No changes are needed in the guest-side mounting logic or the agent — the
extension device discovery via `/sys/block/*/serial` and the systemd units work
the same way regardless of which hypervisor attached the block device.

### Manifest-driven extension discovery via init-data

The current design passes verity parameters for each extension on the kernel
command line. This works well for a small number of extensions but does not
scale: each additional extension adds a long `kata.extension.<name>.verity_params=...`
entry, and the kernel command line has practical size limits.

A future evolution could introduce a **two-phase bootstrap**:

1. **Phase 1 (kernel-params driven)** — the kernel command line carries
   verity parameters for a single, small **manifest extension** image. This
   image is mounted first using the existing mechanism.

2. **Phase 2 (manifest driven)** — the manifest extension contains a
   configuration file (e.g. `extensions.conf`) listing all other extensions with
   their names, verity parameters, and any additional metadata. The mount
   script reads this file and mounts the remaining extensions accordingly.

This approach has several advantages:

- **Kernel command line stays fixed-size** regardless of how many extensions are
  composed.
- **Attestation is simplified** — the TEE firmware measures one manifest
  hash on the command line; the verifier only needs to validate that single
  hash. The chain of trust extends from the measured kernel command line to
  the verified manifest to the verified extensions.
- **Richer metadata** — the manifest can carry structured information beyond
  verity parameters: extension ordering constraints, version requirements,
  policies, or init-data payloads.

The design of the systemd units and mount script can support both modes:
if per-extension verity parameters are present on the kernel command line, they
are used directly (current behavior); if a manifest extension is present, it
is consulted for the remaining extensions. This makes the manifest mode an
additive, backward-compatible evolution.

### Extension versioning and attestation

Future work may add version metadata to guest extension images, enabling the
attestation flow to verify not just integrity (via dm-verity) but also that
the specific expected version of each component is present.

## References

- [Kata Containers architecture](../architecture)
- [dm-verity documentation](https://docs.kernel.org/admin-guide/device-mapper/verity.html)
- [EROFS filesystem](https://docs.kernel.org/filesystems/erofs.html)
- [systemd-sysext](https://www.freedesktop.org/software/systemd/man/latest/systemd-sysext.html)
- [bootc](https://github.com/containers/bootc)
- [ostree](https://ostreedev.github.io/ostree/)

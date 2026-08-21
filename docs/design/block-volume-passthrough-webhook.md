# Block Volume Passthrough via Admission Webhook

## Motivation

Today, a `PersistentVolumeClaim` (PVC) with `volumeMode: Filesystem` is
surfaced inside a Kata guest via virtio-fs. virtio-fs shares the host
filesystem into the guest, which has two consequences for stateful
multi-tenant workloads:

1. **Filesystem resources leak to the host.** Inodes, disk space, and dentry
   / inode slab cache on the host grow proportional to guest filesystem
   activity. A single noisy tenant can exhaust host-side quotas. This
   overlaps with [#12203](https://github.com/kata-containers/kata-containers/issues/12203),
   which frames the per-pod accounting bypass facet; the broader cross-pod
   noisy-neighbor angle is not addressed.

2. **I/O performance cost.** virtio-fs imposes a significant latency and
   throughput penalty versus direct block I/O.

For use cases such as Database-as-a-Service on Kubernetes, both properties
are blockers: tenants are long-lived, write real volume, and are sensitive to
both isolation and latency.

[Direct-Assigned Volume (DAV)](./direct-blk-device-assignment.md) addresses a
related problem by letting CSI drivers hand a block device directly to the
Kata agent. However, DAV requires the CSI driver to be modified to call
`kata-runtime direct-volume add`. A cluster operator who wants to use an
unmodified CSI driver (ceph-csi, OpenEBS, Longhorn, EBS, etc.) does not have
a path to block-device-backed PVCs today.

This document proposes a complementary mechanism that uses a Kata mutating
admission webhook to handle the kata-specific wiring, so that any CSI driver
producing a standard `volumeMode: Filesystem` PVC can be used without
modification.

## Proposed Solution

A mutating admission webhook intercepts PVCs and Pods opted into block
passthrough, transforms them into `volumeMode: Block` volumes with
auto-mount annotations, and lets the existing runtime-rs shim and agent
handle the rest.

### End user interface

The user writes a normal `volumeMode: Filesystem` PVC and a normal pod with
`volumeMounts`. Opt-in is a single label on the PVC (or on its StorageClass,
so all PVCs in that class inherit the behavior):

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: app-data
  labels:
    kata.io/block-passthrough: "true"
spec:
  accessModes: [ReadWriteOnce]
  volumeMode: Filesystem       # stays Filesystem from the user's perspective
  storageClassName: ceph-rbd
  resources:
    requests:
      storage: 10Gi
---
apiVersion: v1
kind: Pod
metadata:
  name: app
spec:
  runtimeClassName: kata-qemu-runtime-rs
  containers:
  - name: app
    image: postgres
    volumeMounts:
    - name: data
      mountPath: /var/lib/postgresql/data
  volumes:
  - name: data
    persistentVolumeClaim:
      claimName: app-data
```

No annotations, no `volumeDevices`, no kata-specific fields in the user's
manifests.

## Assumptions and Limitations

1. **`ReadWriteOnce` only.** Block passthrough is unsafe to share across
   nodes. The webhook rejects PVCs with `ReadWriteMany` when the label is
   present.
2. **`runtime-rs` shim only.** The Go shim does not implement block volume
   auto-mount. Pods must use a runtime class backed by `runtime-rs`.
3. **Filesystem mode at the container level.** The container sees a normal
   filesystem mount at the path specified in `volumeMounts[*].mountPath`.
   Raw block-device containers (`volumeDevices` authored by the user) are a
   separate use case and are not mutated by the webhook.
4. **Default filesystem is `ext4`.** Other filesystems are supported by the
   agent; the webhook currently injects `ext4` as the default. Per-PVC
   override via annotation is a possible extension.

## Implementation Details

### Webhook

A Go admission server implementing two webhook endpoints:

- `/mutate-pvc` — intercepts `PersistentVolumeClaim` create:
  - Requires label `kata.io/block-passthrough: "true"` on the PVC or its
    StorageClass.
  - Rejects `ReadWriteMany` access modes.
  - Rewrites `spec.volumeMode` from `Filesystem` to `Block`.

- `/mutate-pod` — intercepts `Pod` create:
  - Scopes to pods with a Kata `runtimeClassName`.
  - For each `volumeMounts` entry whose backing PVC has the
    `kata.io/block-passthrough` label:
    - Removes the entry from `volumeMounts`.
    - Adds an equivalent entry to `volumeDevices` with
      `devicePath: /dev/kata-vol-<name>`.
    - Adds pod annotations:
      - `io.katacontainers.volume.kata-vol-<name>.mount_path` ← original
        `mountPath`.
      - `io.katacontainers.volume.kata-vol-<name>.fs_type` ← `ext4`
        (default).
  - Applies to both `initContainers` and `containers`.

The webhook is stateless — it only inspects the incoming admission request
and the referenced PVC. No on-disk state, no host-side filesystem artifacts.

### Runtime-rs shim

A new helper `build_block_automount` in
`src/runtime-rs/crates/runtimes/virt_container/src/container_manager/container.rs`
runs during container creation. For each block device attached to the
container, it:

1. Derives `dev_name` from the container-side device path (e.g.
   `/dev/kata-vol-data` → `kata-vol-data`).
2. Looks up pod annotations:
   - `io.katacontainers.volume.<dev_name>.mount_path`
   - `io.katacontainers.volume.<dev_name>.fs_type`
3. If both are present, constructs a `Storage` gRPC object (the same type
   used by DAV) with:
   - `source` = guest-side block device path.
   - `mount_point` = value from `mount_path` annotation.
   - `fstype` = value from `fs_type` annotation.
4. Passes the `Storage` object to the agent alongside existing mounts.

No new RPCs are introduced. The shim reuses the existing `Storage` message
that DAV and other mount sources already populate.

### Kata agent

A new helper `ensure_filesystem` in `src/agent/src/storage/mod.rs` runs
before mounting a block-device `Storage` entry:

1. Runs `blkid <device>`.
2. If a filesystem is detected: runs `fsck.<fstype> -p <device>`. Exit codes
   0 (clean) and 1 (errors corrected) proceed; higher codes abort with an
   error.
3. If no filesystem is detected: runs `mkfs.<fstype> <device>`.
4. Proceeds with the normal mount flow.

This replicates the `NodeStageVolume` responsibility that CSI drivers
normally perform on the host, but does it inside the guest where the raw
device now lives. The operation is idempotent and safe to call on every
mount.

## Step-by-step walk-through

Given the PVC and Pod in the "End user interface" section, the flow is:

1. **User applies the PVC.** The admission webhook `mutatePVC` sees the
   `kata.io/block-passthrough` label, verifies `ReadWriteOnce`, and patches
   `spec.volumeMode` from `Filesystem` to `Block`. The CSI driver provisions
   a raw block volume.

2. **User applies the Pod.** The admission webhook `mutatePod` sees the
   Kata runtime class, finds the `data` volume references a
   block-passthrough PVC, and patches the container spec:
   - `volumeMounts: [{name: data, mountPath: /var/lib/postgresql/data}]`
     → `volumeDevices: [{name: data, devicePath: /dev/kata-vol-data}]`
   - Adds pod annotations:
     - `io.katacontainers.volume.kata-vol-data.mount_path: /var/lib/postgresql/data`
     - `io.katacontainers.volume.kata-vol-data.fs_type: ext4`

3. **Kubelet attaches the volume.** The CSI driver exposes the block device
   to the node and Kubelet passes the raw device into the pod spec as a
   `volumeDevice`.

4. **Kata shim creates the container.** `build_block_automount` reads the
   annotations, builds a `Storage` object for `/dev/kata-vol-data` with
   mount point `/var/lib/postgresql/data` and fstype `ext4`, and sends it
   to the agent.

5. **Kata agent mounts the volume.** `ensure_filesystem` runs: on a fresh
   PVC, `blkid` reports no filesystem, so the agent runs `mkfs.ext4`. On
   subsequent pod starts (after a restart or failover), `blkid` reports
   ext4, so the agent runs `fsck.ext4 -p` before mounting.

6. **Container starts.** The application sees a normal filesystem mount at
   `/var/lib/postgresql/data`, backed by a block device inside the guest
   with no virtio-fs involvement.

## Relationship to DAV

DAV and this proposal both deliver a block-device-backed volume to the
Kata guest; they differ in where the integration code lives.

| | DAV | This proposal |
|---|---|---|
| Where integration lives | CSI driver | Kata admission webhook |
| User's PVC `volumeMode` | `Filesystem` | `Filesystem` (unchanged to user) |
| Effective `volumeMode` | `Filesystem` + host-mount-skip side channel | `Block` (rewritten by webhook) |
| Fresh PVC (no filesystem) | Assumes pre-formatted | `mkfs` on first use |
| Online resize / stats | Supported today | Not in initial scope; can adopt DAV's gRPC APIs later |
| State on host | `mountInfo.json` under `/run/kata-containers/...` | None |

The two approaches produce the same `Storage` gRPC object at the shim-agent
boundary and can coexist without conflict.

## Out of scope

- **Go shim parity.** runtime-rs is upstream's long-term target. The Go
  shim can adopt the same logic as a follow-up if needed.
- **Online resize and stats.** These can be added later by adopting DAV's
  existing gRPC APIs (`ResizeVolume`, `GetVolumeStats`).
- **Encrypted filesystems inside the guest.** Out of scope for the initial
  proposal; compatible as a future extension.
- **Per-PVC filesystem selection.** The webhook currently hardcodes `ext4`.
  A future label or annotation can expose this.

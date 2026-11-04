# Storage Capacity Aware Scheduling

`csi-kata-directvolume` stores volumes on the node where the pod runs. Nodes
can have different amounts of storage, so the scheduler has to know how much
each node can still provide. The driver reports it through the CSI
`GetCapacity` call, the `csi-provisioner` sidecar of every node publishes it as
[`CSIStorageCapacity`](https://kubernetes.io/docs/concepts/storage/storage-capacity/)
objects, and the scheduler only places a pod with an unbound volume on a node
that has enough capacity for it.

This document describes how to configure capacity and how the driver
calculates and enforces it.

## Requirements

- Kubernetes 1.24 or newer, which provides the `storage.k8s.io/v1`
  `CSIStorageCapacity` API.
- A StorageClass with `volumeBindingMode: WaitForFirstConsumer`. The scheduler
  ignores capacity for volumes which are bound immediately.
- The deployment in [`deploy`](../deploy): the `CSIDriver` sets
  `storageCapacity: true` and the `csi-provisioner` sidecars run with
  `--enable-capacity`.

`deploy/deploy.sh` enables capacity tracking when the cluster has the
`CSIStorageCapacity` v1 API, and removes it from the manifests otherwise. Set
`CAPACITY_TRACKING=true` or `CAPACITY_TRACKING=false` to override this.

## Configure capacity pools

A capacity pool is an amount of storage that volumes are reserved from. Pools
are configured per node, and a StorageClass selects one with the
`katacontainers.direct.volume/capacity-pool` parameter:

```yaml
apiVersion: storage.k8s.io/v1
kind: StorageClass
metadata:
  name: csi-kata-directvolume-capacity-sc
parameters:
  katacontainers.direct.volume/volumetype: directvol
  katacontainers.direct.volume/fstype: ext4
  katacontainers.direct.volume/capacity-pool: standard
provisioner: directvolume.csi.katacontainers.io
reclaimPolicy: Delete
volumeBindingMode: WaitForFirstConsumer
```

Pool names follow the syntax of Kubernetes label values: up to 63 characters,
starting and ending with a letter or digit, with `-`, `_` and `.` in between.

### Per-node configuration

The driver container mounts the optional ConfigMap
`kata-directvolume/csi-kata-directvol-capacity` at
`/etc/csi-kata-directvolume/capacity` and reads it with
`--capacity-config-dir`. Every key of the ConfigMap is the name of a node. Its
value lists the pools of that node as `<pool>=<size>` entries, one per line or
separated by commas. Sizes are Kubernetes quantities such as `500Gi` or `2Ti`.
Lines starting with `#` are comments.

For example, to give the `standard` pool 2 GiB on `node-a` and 8 GiB on
`node-b`:

```bash
$ kubectl -n kata-directvolume create configmap csi-kata-directvol-capacity --from-literal=node-a=standard=2Gi --from-literal=node-b=standard=8Gi
```

[`examples/pod-with-capacity`](../examples/pod-with-capacity) contains the
same configuration as a manifest, together with a StorageClass, a PVC and a
pod which use it.

The driver reads the entry of its node when it starts. After changing the
ConfigMap, restart the driver:

```bash
$ kubectl -n kata-directvolume rollout restart daemonset/csi-kata-directvol-plugin
```

The ConfigMap is the recommended source because one object describes the
whole cluster. Any other way to provide a file named after the node in that
directory works too, for example a `hostPath` volume with files written by the
node provisioning tooling.

### Pools for all nodes

Pools which every node has can also be set with the `--capacity=<pool>=<size>`
argument of the `kata-directvolume` container, once per pool. The entries of
the per-node file are added to these pools and replace pools with the same
name. To remove a pool from one node, give it the size `0` in that node's
entry.

### Without capacity pools

When a node has no capacity pool, volumes on it are only limited by the free
space of the filesystem which holds their backing files, as described in
[filesystem free space](#filesystem-free-space). A StorageClass must not select
a pool on such a node.

## How capacity is calculated

### Authoritative capacity source

The configured pool size is the authoritative capacity. A volume reserves its
full size from its pool when `CreateVolume` succeeds and releases it when
`DeleteVolume` succeeds:

```text
pool available = max(0, pool size - sum of the sizes of the volumes in the pool)
```

Reservations are recorded in the state file of the driver
(`--statedir`, `/csi-persist-data/state.json` in the DaemonSet) together with
the pool of each volume, before the volume is reported as created. A restarted
driver therefore restores every reservation and counts each one exactly once.

### Filesystem free space

The free space of the filesystem which holds the backing files is used as an
additional ceiling, so that a pool which is configured larger than its disk
cannot hand out space that does not exist:

```text
filesystem available = free space - sum over the volumes on that filesystem of
                       (volume size - bytes already allocated for its backing file)
available            = max(0, min(pool available, filesystem available))
```

The two volume types allocate their backing files differently, which is why
the allocated part is subtracted from each reservation:

- A `directvol` volume gets a raw disk under `--storagepath` when it is
  created, which `fallocate` allocates completely, right after the capacity
  check. Until then its whole size is held back.
- A `spdkvol` volume gets a sparse backing file under `--spdk-rawpath` when it
  is created. Only the parts which the guest has written are allocated, and the
  rest of the volume is held back.

Space which is allocated already is missing from the free space, so it is not
counted a second time, and space which a sparse file may still use is not
handed out to another volume. Thin provisioning, which would overcommit sparse
files on purpose, is not supported. When `--storagepath` and `--spdk-rawpath`
are on the same filesystem, the volumes of both types share its free space.

### What `GetCapacity` reports

`GetCapacity` uses the same calculation as the admission check of
`CreateVolume`, for the parameters, capabilities and topology in the request:

| Request | `available_capacity` | `maximum_volume_size` |
| --- | --- | --- |
| Pool configured on this node | `available` | `min(available, --max-volume-size)` |
| No pools on this node, no pool selected | `filesystem available` | `min(available, --max-volume-size)` |
| Pool not configured on this node | 0 | 0 |
| No pool selected, but the node has pools | 0 | 0 |
| Topology of another node | 0 | 0 |
| Block access type or multi-node access mode | 0 | 0 |
| Malformed pool name, or two different pools selected | `INVALID_ARGUMENT` error | |

Both values are never negative, and `maximum_volume_size` never exceeds
`available_capacity`. The scheduler compares the size of a volume with
`maximumVolumeSize` of the `CSIStorageCapacity` object.

### What `CreateVolume` enforces

`CreateVolume` resolves the pool exactly like `GetCapacity` and checks the
request before any backend resource, such as an SPDK backing file or `bdev`,
is created:

| Condition | Error code |
| --- | --- |
| Larger than `--max-volume-size` | `OUT_OF_RANGE` |
| Larger than the available capacity | `RESOURCE_EXHAUSTED` |
| Pool not configured, or not selected although the node has pools | `RESOURCE_EXHAUSTED` |
| Requisite topology does not contain this node | `RESOURCE_EXHAUSTED` |
| Malformed pool, unsupported capability or invalid capacity range | `INVALID_ARGUMENT` |

With `WaitForFirstConsumer`, `RESOURCE_EXHAUSTED` makes the `csi-provisioner`
give the volume back to the scheduler, which then picks another node. A
repeated `CreateVolume` for an existing volume returns that volume and does not
reserve capacity again; a request which does not match the existing volume
fails with `ALREADY_EXISTS`.

### Topology

The driver reports the topology segment `topology.directvolume.csi/node=<node>`
for every node and as the accessible topology of every volume, so the
persistent volume can only be used on the node which stores it. The
`csi-provisioner` sidecars run with `--node-deployment` and
`--strict-topology`, so each one only provisions volumes for its own node and
`CreateVolume` receives that node as the requisite topology.

### Publication

Each `csi-provisioner` sidecar calls `GetCapacity` for every
`WaitForFirstConsumer` StorageClass of the driver, with the topology of its
node, every `--capacity-poll-interval` (one minute in the DaemonSet) and right
after it created or deleted a volume. It publishes the result as one
`CSIStorageCapacity` object per StorageClass and node in the
`kata-directvolume` namespace. With `--capacity-ownerref-level=1` the objects
are owned by the DaemonSet: they survive a restart of the driver pods, are
updated by the new pods and are removed together with the DaemonSet.

The `csi-provisioner` service account gets the permissions which this needs,
`CSIStorageCapacity` access in its namespace and `get` on its own pod, from the
RBAC rules of the external-provisioner release, which `deploy/rbac-deploy.sh`
installs.

## Changing the configuration

- **Growing a pool** makes the extra space available after the driver restart.
- **Shrinking a pool** below what is reserved keeps the existing volumes. The
  pool reports zero capacity until enough volumes are deleted, and the driver
  logs a warning when it starts.
- **Removing a pool** keeps the existing volumes and their reservations. New
  volumes cannot be created in that pool, and deleting the volumes works as
  usual. Renaming a pool is the same as removing it and adding a new one.
- **Introducing pools** on a node which had none: volumes which already exist
  are not charged to any pool, but the filesystem ceiling keeps counting them.
  StorageClasses which do not select a pool can no longer place new volumes on
  that node.
- **Upgrading** from a driver version without capacity pools keeps all
  existing volumes usable. Their state entries have no pool, and they are only
  counted by the filesystem ceiling.
- **StorageClasses with `volumeBindingMode: Immediate`** keep working. The
  `csi-provisioner` does not publish capacity for them, but `CreateVolume`
  enforces the same limits.

## Compatibility

| Component | Supported versions |
| --- | --- |
| Kubernetes | 1.24 or newer for capacity tracking. Older clusters work without it. |
| external-provisioner | v3.6.0, as deployed, and newer releases, which accept the same arguments. |
| Kata Containers | `runtime-rs` with QEMU or Dragonball, as for all direct volumes. |

The end-to-end test in [`test/e2e`](../test/e2e/run-capacity-e2e.sh) has been
run with Kubernetes 1.37 and Kata Containers 4.3.1, with `runtime-rs` and QEMU
or Dragonball, and with external-provisioner v3.6.0 and v6.4.0. To use another
external-provisioner release, deploy with `CSI_PROVISIONER_TAG`, which also
selects its RBAC rules.

## Limitations

- The per-node configuration is only read when the driver starts.
- `CSIStorageCapacity` objects of a node which leaves the cluster remain until
  the DaemonSet is deleted. The scheduler ignores them because no node matches
  their topology.
- Sparse SPDK backing files cannot be overcommitted.

## Testing

| Test | Command | Needs |
| --- | --- | --- |
| Unit, gRPC and manifest tests | `go test ./...` | Go |
| CSI sanity | `test/sanity/run-csi-sanity.sh` | root, `csi-sanity` |
| Two-node end-to-end test | `test/e2e/run-capacity-e2e.sh` | Docker, `kind`, `kubectl`, `helm`, `jq`, `yq`, `/dev/kvm` |

The end-to-end test creates a `kind` cluster with two worker nodes, installs
Kata Containers with `kata-deploy` and gives one pool 2 GiB on the first and
8 GiB on the second worker. It checks that the capacity is published, that a
4 GiB volume is placed on the second worker, that a 5 GiB volume stays pending
while the first one exists and gets provisioned once it is deleted, and that a
restart of the driver keeps all reservations. It writes a report with the
versions, the published capacity of every phase and the logs to the
`ARTIFACTS` directory. To run it on an existing cluster with Kata Containers,
set `USE_EXISTING_CLUSTER=true`, `NODE_A` and `NODE_B`.

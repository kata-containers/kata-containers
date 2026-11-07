# Storage capacity and scheduling

Direct volumes use node-local backing storage. Kubernetes therefore needs both
node topology and a current capacity report to avoid placing a Pod where its
PVC cannot be provisioned.

> **Current status:** the checked-in deployment does not yet enable this path.
> `CSIDriver.spec.storageCapacity` is `false`, the provisioner's
> `--enable-capacity` argument is commented out, and the example StorageClasses
> use `Immediate` binding. The unit and integration tests describe and validate
> the capacity contract without changing that deployment behavior. Completing
> the driver and manifest work is tracked in issue #14151.

## How capacity is reported

The driver accepts a logical capacity quota for each named pool:

```console
directvolplugin --capacity=fast=8Gi --max-volume-size=8Gi ...
```

The StorageClass parameter `kind: fast` selects that pool in a `GetCapacity`
call. With capacity tracking configured, the driver calculates:

```text
available capacity = configured pool capacity - sum of recorded volume sizes in that pool
maximum volume size = min(configured per-volume maximum, available capacity)
```

All persisted volumes in the same pool count toward the reservation, including
multiple PVs on one node. Volumes in other pools do not reduce this pool's
capacity. This is a logical reservation total, not a measurement of filesystem
free space; sparse backing files are accounted for by their requested size.

There are two important fallback cases in the current implementation:

- If capacity tracking is configured but `kind` is missing or unknown,
  `GetCapacity` returns zero available capacity and zero maximum volume size.
  Once scheduler capacity checking is enabled, that node cannot satisfy a
  positive request.
- If no `--capacity` pool is configured, `GetCapacity` reports
  `--max-volume-size` as both values. This is only a per-volume limit fallback;
  it is **not** the node's free disk space and must not be treated as a safe
  physical-capacity report.

The current multi-pool provisioning path is still preliminary: `CreateVolume`
does not propagate `kind` into `VolumeContext`, and an omitted pool can be
selected from the configured map. Use a single explicit pool when exercising
the existing code. The issue #14151 core work must make pool selection and SPDK
quota enforcement consistent before capacity publication is enabled by
default.

## How Kubernetes uses the report

Capacity-aware placement requires all of the following:

1. the node driver is configured with an appropriate logical capacity;
2. external-provisioner runs with `--enable-capacity` (and, for this DaemonSet,
   `--capacity-ownerref-level=1`);
3. `CSIDriver.spec.storageCapacity` is `true`;
4. the StorageClass uses `volumeBindingMode: WaitForFirstConsumer` and the
   intended `kind` parameter; and
5. external-provisioner publishes `CSIStorageCapacity` objects from
   `GetCapacity` responses, scoped by the node topology.

The scheduler matches an unbound PVC and candidate node to those objects. It
uses `maximumVolumeSize` when that field is present, otherwise `capacity`, and
filters the node when the requested volume is larger than the applicable
limit. Insufficient capacity is therefore a normal non-match, not a CSI RPC
error. When capacity checking is enabled, the absence of a matching sufficient
`CSIStorageCapacity` object also makes the node ineligible.

Setting `CSIDriver.spec.storageCapacity` to `false` has different semantics:
the scheduler skips this capacity check. Publishing capacity objects alone is
not enough to enable filtering.

The external-provisioner polling interval controls how quickly published values
converge after provisioning or deletion. The two-node integration scenario in
`test/capacity_scheduling_integration_test.go` documents a local kind setup and
uses a polling interval of 10 seconds or less.

## Stale or missing capacity information

A `CSIStorageCapacity` object is a snapshot, not a reservation. Another PVC can
consume space after the snapshot is published, so even an otherwise valid
scheduler decision can be stale by the time `CreateVolume` reaches a node.

The required fallback is to revalidate at provisioning time:

1. the driver's configured-capacity check rejects a request that no longer
   fits with `ResourceExhausted`, without recording another reservation;
2. external-provisioner and Kubernetes delayed binding reset the tentative
   node selection and retry; and
3. fresh capacity information excludes the exhausted node, allowing another
   node with room to be selected.

Consequently, a stale report must never be interpreted as proof that creation
will succeed. Conversely, a successful scheduler decision does not reserve
capacity until `CreateVolume` succeeds.

If capacity publication temporarily fails, previously published information
can likewise be old. Operators should check the provisioner logs and the
`CSIStorageCapacity` values rather than assuming they are current. Deleting or
losing all matching objects while `storageCapacity=true` is conservative for
new delayed-binding claims: no candidate node has a matching sufficient report.
Turning `storageCapacity` off instead removes filtering and falls back to
provisioning retries, which can select a full node.

The unit tests cover sufficient and insufficient reports, the missing-pool and
unconfigured fallbacks, accumulation of multiple PVs, and stale-report
revalidation followed by selection of the remaining node. The opt-in
integration test covers scheduling outcome, exhaustion, and capacity recovery
on a real two-node cluster.

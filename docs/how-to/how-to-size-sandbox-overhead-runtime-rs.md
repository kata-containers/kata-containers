# How to size `overhead_*` for runtime-rs sandbox sizing

This document explains how `overhead_vcpus` and `overhead_memory` are expected
to be used in runtime-rs.

> [!WARNING]
> For runtime-rs, using `static_sandbox_resource_mgmt` is the recommended mode.
> Disabling it is not recommended for production sandbox sizing.

> [!IMPORTANT]
> For correct and predictable Kata sandbox sizing in Kubernetes, workload CPU
> and memory limits **must** be set. Without limits, runtime-rs falls back to
> `default_vcpus` and `default_memory`, which is a compatibility fallback and
> not the intended production sizing model.

## Why these fields exist

In runtime-rs, static sandbox sizing is enabled by default. Kata must pick VM resources before
starting the workload. In Kubernetes, pod limits represent workload resources,
but the VM also needs extra resources for guest/kernel/runtime overhead.

`overhead_vcpus` and `overhead_memory` represent that extra budget.

## Sizing model

With runtime-rs static sandbox sizing, Kata uses:

- If workload limits are present:
  - `vm_vcpus = requested_vcpus + overhead_vcpus`
  - `vm_memory = requested_memory + overhead_memory`
- If workload limits are not present:
  - `vm_vcpus = default_vcpus`
  - `vm_memory = default_memory`

In other words, `default_*` is the fallback for "no limits", while
`overhead_*` is the additive budget for "limits are set".
For CPU, runtime-rs sums workload and overhead values, and if the computed
result is fractional it is rounded up to the next integer (`ceil`), since VMMs
expose integer vCPU counts. A minimum of `1` vCPU is enforced for the
limit-driven path, including the `0 + 0` edge case.

## `podFixed` as a sizing function

Treat `RuntimeClass.overhead.podFixed` as a function of expected VM size:
larger VMs usually need larger overhead budgets, for both static and dynamic
allocation environments.

Operationally, this usually leads to one of two models:

- Single runtime class: one conservative `podFixed` value that works across all
  expected workload sizes.
- Multiple runtime classes (for example S/M/L/XL): each class has a tailored
  `podFixed` and runtime profile for tighter node-level accounting.

Kata cannot ship a single correct value for this function, because it depends on
a large number of deployment-specific factors, including:

- the hypervisor in use (each has a different memory/CPU footprint),
- the file-sharing mechanism (`virtio-fs` vs. others),
- the presence of CoCo guest components,
- the VM image in use (our released images, or downstream-modified ones),
- hardware features such as GPUs (or anything else requiring large DMA buffers).

These factors, the inherent brittleness of overhead measurements, and how much
headroom a cluster owner is willing to "waste" to guarantee stable operation,
all feed into the value. Downstream operators should therefore measure and tune
this function for their own deployments.

## Recommended operator/admin workflow

The Kubernetes documentation defines `RuntimeClass.overhead.podFixed` as:

> podFixed represents the fixed resource overhead associated with running a pod.

For Kata, that overhead has two parts: the *guest-side* overhead (the extra
CPU/memory the VM needs on top of the workload) and the *host-side* overhead
(the runtime, hypervisor, and helper processes running on the node). `podFixed`
must account for **both**, while Kata `overhead_*` accounts for the guest-side
part only.

A practical workflow is therefore:

1. Estimate (or measure) the guest-side overhead. Kata profiles ship with a
   starting value, but you should refine it for your environment.
2. Set Kata `overhead_*` per runtime profile to that guest-side value.
3. Estimate (or measure) the host-side overhead.
4. Set `RuntimeClass.overhead.podFixed` to the sum of the guest-side and
   host-side overhead. This naturally keeps `podFixed` higher than `overhead_*`.
5. Validate with representative workloads (small/medium/large). As rough
   starting points for the measurements:
   - guest-side overhead: subtract a container's used memory (for example,
     `free` inside the container) from the nominal VM size;
   - host-side overhead: subtract the nominal VM size from the pod's host
     cgroup usage, for example
     `cat /sys/fs/cgroup/kubepods.slice/**/memory.current`.

For production-oriented Kata deployments, assume users provide workload limits.
The no-limits path exists as a compatibility fallback, not as the primary sizing
model.

Kata profiles initialize `overhead_*` to values derived from Pod Overhead (for
example, 80% for CPU and memory), but this is only a policy input and should be
tuned by downstream operators and admins.

## Who sets what: admin vs user

In many environments, the "admin" and the "user" are different personas. In
smaller environments they may be the same person or team.

- Admin/operator responsibilities:
  - Set runtime defaults (`default_*`) and overhead values (`overhead_*`).
  - Set and maintain `RuntimeClass.overhead.podFixed`.
  - Provide runtime classes that users can select per workload profile.
  - Ensure those policies are aligned for each runtime profile.
  - Validate behavior with representative workloads and adjust if needed.

- User/application responsibilities:
  - Set pod/container CPU and memory limits for workload intent.
  - Use the runtime class provided by admins for the workload profile.
  - Avoid relying on default sizing when deterministic resources are required.

## Example 1: limits set on both CPU and memory

**Scenario intent:** show the standard production case with explicit workload limits.

**Consequence:** users get predictable sizing plus admin-defined overhead budget.

**`RuntimeClass.overhead.podFixed` relationship:** `podFixed` should be higher than
`overhead_*`, since `podFixed` must include host-side runtime components.

Given the runtime profile:

- `default_vcpus = 2`
- `default_memory = 1024`
- `overhead_vcpus = 0.5`
- `overhead_memory = 128`

And the matching `RuntimeClass.overhead.podFixed`:

- `cpu = 600m` (`0.6`)
- `memory = 160Mi`

Workload limits:

- CPU quota/period equivalent to `1.5 vCPUs`
- memory limit `600 MiB`

Kata VM sizing (guest side):

- `vm_vcpus = 1.5 + 0.5 = 2.0`
- `vm_memory = 600 + 128 = 728 MiB`

Kubernetes accounting for the whole pod (`sum(limits) + podFixed`):

- `pod_cpu = 1.5 + 0.6 = 2.1`
- `pod_memory = 600 + 160 = 760 MiB`

Note that `podFixed` (`160Mi`) is higher than `overhead_memory` (`128`), since it
must also cover the host-side runtime components that live outside the VM.

## Example 2: partial limits (split by dimension)

**Scenario intent:** show what happens when only one limit is provided.

**Consequence:** once any limit exists, overhead logic applies to both dimensions.

**`RuntimeClass.overhead.podFixed` relationship:** same rule as Example 1;
`podFixed` should remain higher than `overhead_*`.

Given:

- `default_vcpus = 2`
- `default_memory = 1024`
- `overhead_vcpus = 0.5`
- `overhead_memory = 128`

### 2A. Memory limit only

Workload sets:

- memory limit = `512 MiB`
- no CPU limit

Result:

- CPU is rounded up for boot: `vm_vcpus = ceil(0 + 0.5) = 1`
- Memory uses overhead formula: `vm_memory = 512 + 128 = 640 MiB`

### 2B. CPU limit only

Workload sets:

- CPU quota/period equivalent to `1.5 vCPUs`
- no memory limit

Result:

- CPU uses overhead formula: `vm_vcpus = 1.5 + 0.5 = 2.0`
- Memory still uses overhead baseline: `vm_memory = 0 + 128 = 128 MiB`

This is the reason workload memory limits **must** be set (see the note at the
top of this document): with a CPU limit but no memory limit, the VM is sized
with `overhead_memory` only, which is almost certainly too small to run a real
workload. It is the explicit overhead baseline, not a default fallback to
`default_memory`. As a safety net, if the computed sandbox memory would be `0`
(for example, a CPU-only workload with `overhead_memory = 0`), runtime-rs fails
early with an actionable error instead of booting an unusable VM.

This mirrors runtime-rs behavior: once limits are present for a sandbox, overhead
is applied on both dimensions, and any missing dimension uses `0 + overhead_*`
(with fractional CPU results rounded up).

## Example 3: `overhead_* = 0` (zero-overhead model)

**Scenario intent:** user-driven exact workload sizing by setting `overhead_* = 0`.

**Consequence:** users get exactly requested VM sizes when limits are set, but they
are accountable for accounting workload-related overhead in those limits.

**`RuntimeClass.overhead.podFixed` relationship:** `podFixed` is still required to
cover host-side resource usage (not guest-side), and should be tuned
independently.

Some deployments may choose to set:

- `overhead_vcpus = 0`
- `overhead_memory = 0`

With:

- `default_vcpus = 2`
- `default_memory = 1024`

### 3A. Limits set on both dimensions

Workload limits:

- CPU = `1.5 vCPUs`
- memory = `600 MiB`

Result:

- `vm_vcpus = 1.5 + 0 = 1.5`
- `vm_memory = 600 + 0 = 600 MiB`

### 3B. No limits

Result:

- `vm_vcpus = default_vcpus = 2`
- `vm_memory = default_memory = 1024 MiB`

This keeps defaults as fallback only, while limit-driven sizing becomes purely
workload-based.

## Example 4: no limits (fallback path)

**Scenario intent:** show compatibility fallback behavior when users do not
provide limits.

**Consequence:** VM sizing comes from admin-defined defaults. This is acceptable
for basic workloads and testing, **but not the intended production sizing
posture**.

**`RuntimeClass.overhead.podFixed` relationship:** in this case, `podFixed`
should be higher than the effective default baseline (`default_*`) to account
for host-side components as well. Kubernetes does not know Kata `default_*`
values; if `podFixed` is too low, host-side usage can exceed the pod budget and
the pod may be killed.

Given:

- `default_vcpus = 2`
- `default_memory = 1024` (MiB)
- `overhead_vcpus = 0.5`
- `overhead_memory = 128` (MiB)

Pod/container limits are not set.

Result:

- VM boots with `2 vCPUs` and `1024 MiB`.
- `overhead_*` is not used in this case.

## Runtime profile snippet

```toml
[hypervisor.qemu]
default_vcpus = 2
default_memory = 1024
overhead_vcpus = 0.5
overhead_memory = 128
```

## Helm examples

With kata-deploy Helm, the recommended pattern is to set `overhead_*` in a
runtime `dropIn` and set the corresponding `RuntimeClass.overhead.podFixed`
to a higher value in the same values file.

For runtime-rs, `static_sandbox_resource_mgmt` is already enabled by default, so
these examples focus on `overhead_*` and related policy values.

### Example A: custom runtime profile

```yaml
customRuntimes:
  enabled: true
  runtimes:
    my-qemu-runtime-rs:
      baseConfig: "qemu"
      dropIn: |
        [hypervisor.qemu]
        default_vcpus = 2
        default_memory = 1024
        overhead_vcpus = 0.5
        overhead_memory = 128
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: kata-my-qemu-runtime-rs
          labels:
            app.kubernetes.io/managed-by: kata-deploy
        handler: kata-my-qemu-runtime-rs
        overhead:
          podFixed:
            cpu: "600m"
            memory: "160Mi"
        scheduling:
          nodeSelector:
            katacontainers.io/kata-runtime: "true"
```

In this example:

- Kata overhead used for VM sizing is `0.5 vCPU` and `128Mi`.
- Kubernetes scheduler/accounting overhead is `600m` and `160Mi`.
- The gap (`podFixed` > `overhead_*`) leaves extra budget for components outside
  the guest workload cgroup model.

### Example B: override a default shim with `shims.<shim>.dropIn`

If you do not need a new runtime class, you can patch an existing runtime-rs
shim directly:

```yaml
shims:
  qemu:
    enabled: true
    dropIn: |
      [hypervisor.qemu]
      overhead_vcpus = 0.5
      overhead_memory = 128
```

This updates Kata sizing behavior for that shim. If you also control the
runtime class YAML externally, keep `podFixed` greater than `overhead_*` under
the same sizing policy.

## Kubernetes alignment notes

- `RuntimeClass.overhead.podFixed` and Kata `overhead_*` should be managed by
  the same operator/admin policy, with `podFixed` set higher than `overhead_*`.
- Mismatched values can produce surprising behavior under pressure.
- Upstream runtime-rs does not auto-fetch RuntimeClass overhead from Kubernetes;
  the configured `overhead_*` values are the source used for VM sizing.

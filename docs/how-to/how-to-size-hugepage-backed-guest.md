# How to size a huge page backed guest from its pod

## How the guest is sized

With `enable_hugepages_pod_sizing = true`, a huge page backed guest
(`enable_hugepages = true`, `static_sandbox_resource_mgmt = true`,
`sandbox_cgroup_only = true`) is sized from the pod's `hugepages-<size>`
reservation. The CRI does not pass that reservation to the runtime, so the
runtime reads the `hugetlb.<size>.max` the kubelet set on the pod's cgroup,
which includes the runtime class's pod overhead. The host must use cgroup v2.

Inside the guest, a container's own `hugepages-<size>` is its memory
ceiling. Its `memory` limit only covers what the sandbox uses on the host,
so it can stay small and the pod still be Guaranteed. A container without
a reservation keeps its `memory` limit as its ceiling.

The guest also needs memory of its own, which grows with its size. Declare
it as `hugepages-<size>` in the runtime class's pod overhead, or as a pod
level value above the containers' sum:

```yaml
kind: RuntimeClass
handler: kata-qemu
overhead:
  podFixed:
    cpu: "1"
    memory: 1Gi
    hugepages-1Gi: 2Gi
```

A pod in this class whose containers reserve 14Gi and 2Gi gets an 18Gi
guest, with ceilings of 14Gi and 2Gi. The formula for the guest's own
share is in the comment on `enable_hugepages_pod_sizing`.

To read a container's ceiling and to see a pod-level OOM:

```bash
kubectl exec <pod> -c <container> -- cat /sys/fs/cgroup/memory.max
kubectl exec <pod> -c <container> -- dmesg | grep oom_memcg
# pod-level: oom_memcg=/kubepods-pod<uid>.slice/cri-containerd (the pod's cgroup)
```

The Go runtime also has the agent bound the pod's containers together on
their parent cgroup, so the OOM killer of that cgroup stops a pod that
outgrows the guest. An agent older than the runtime ignores that bound.

The option works with QEMU and Cloud Hypervisor in the Go runtime and with
Dragonball in runtime-rs.

## What is refused

Refused when the configuration loads:

- the option without `static_sandbox_resource_mgmt`, or without
  `sandbox_cgroup_only` in the Go runtime;
- the option with a VM template, or with the VM cache in the Go runtime;
- the option with a hypervisor other than Dragonball in runtime-rs.

Refused before the VM starts:

- a host on cgroup v1;
- a pod cgroup whose `hugetlb.<size>.max` is missing or sets no finite limit,
  as outside a kubelet;
- a pod that reserved none of the guest's page size;
- a reservation under 1024 MiB, or above `default_maxmemory` when set.

Refused when a container is created or updated:

- a huge page backed `emptyDir`, since huge pages inside the guest are not
  supported in such a runtime class;
- huge pages of another page size than the VM's;
- ceilings that together do not fit the guest, with the amount the pod is short;
- containers that all carry the same pod level value past the VM's size.

A container reserving 16Gi in a class without huge pages in its overhead:

```
container app brings the memory ceilings of the pod's containers to 16384 MiB, but the 16384 MiB guest holds 15129 MiB for them: the pod needs 1255 MiB more of the VM's huge pages, on the pod or as the runtime class's pod overhead
```

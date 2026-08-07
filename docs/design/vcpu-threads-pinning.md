# Design Doc for Kata Containers VCPUs Pinning Feature

## Background
By now, vCPU threads of Kata Containers are scheduled randomly to CPUs. And each pod would request a specific set of CPUs which we call it CPU set (just the CPU set meaning in Linux cgroups).

Every vCPU thread the CPU set can back with a CPU of its own is pinned to it, to reduce the cost of random scheduling. A thread beyond that keeps the CPU set as its affinity, as pinning it would mean sharing a CPU with the vCPU that holds it.

## Detailed Design

### Passing Config Parameters
Two ways are provided to use this vCPU thread pinning feature: through `QEMU` configuration file and through annotations. Finally the pinning parameter is passed to `HypervisorConfig`.

### Related Linux Thread Scheduling API

| API Info          | Value                                                     |
|-------------------|-----------------------------------------------------------|
| Package           | `golang.org/x/sys/unix`                                     |
| Method            | `unix.SchedSetaffinity(thread_id, &unixCPUSet)`             |
| Official Doc Page | https://pkg.go.dev/golang.org/x/sys/unix#SchedSetaffinity |

### When is VCPUs Pinning Checked?

As shown in Section 1, the placement follows `num(CPUs in CPU set)`: how many vCPU threads can be given a CPU of their own depends on it, and a thread that loses its CPU because the set shrank goes back to being scheduled over the whole set.
So when may `num(CPUs in CPU set)` change? There are 5 possible scenes:

| Possible scenes                   | Related Code                               |
|-----------------------------------|--------------------------------------------|
| when creating a container         | File Sandbox.go, in method `CreateContainer`  |
| when starting a container         | File Sandbox.go, in method `StartContainer`   |
| when deleting a container         | File Sandbox.go, in method `DeleteContainer`  |
| when updating a container         | File Sandbox.go, in method `UpdateContainer`  |
| when creating multiple containers | File Sandbox.go, in method `createContainers` |

### Core Pinning Logics

We can split the whole process into the following steps. Related methods are `checkVCPUsPinning` and `resetVCPUsPinning`, in file Sandbox.go.
![](arch-images/vcpus-pinning-process.png)

### NUMA-Aware Placement

When the sandbox has a guest NUMA topology (`enable_numa`), `checkVCPUsPinning` hands the work to `checkVCPUsPinningNUMA`, which keeps every vCPU thread on the host NUMA node that the vCPU's guest node maps to, so that a vCPU and the memory of its node stay local to each other.

Inside that node, the placement depends on what the sandbox owns:

| Condition | Placement |
|-----------|-----------|
| The sandbox owns its CPU set and a CPU of it is still unclaimed | the vCPU thread is pinned to a host CPU of its own, taken from its node, or borrowed from the CPUs no node claimed once every node has served its own vCPUs |
| Otherwise | the vCPU thread gets the CPUs of its node as an affinity mask |

The two conditions are evaluated per vCPU, so a CPU set that cannot back the whole guest pins the vCPUs it can and leaves the surplus threads to the host scheduler within their node. That is the common case under static sizing, which boots the guest with `default_vcpus` on top of the pod's CPU limit while the kubelet reserves only the limit. Sharing a node beats the alternative of pinning two vCPU threads to one CPU, which would make them time-share it for good; the scheduler can instead move such a thread to whichever CPU of the node has slack.

The mask is also all a sandbox that does not own its CPU set can get — no cpuset was assigned to it, so the CPUs are derived from the NUMA topology and shared with every other sandbox on the host, which gives no basis for claiming individual CPUs: each sandbox would claim the same ones and its vCPU threads would then time-share CPUs with the vCPU threads of the others. A CPU set larger than the vCPU count is fine either way: the surplus CPUs simply stay unused by this sandbox.

See [How to use NUMA with Kata](../how-to/how-to-use-numa-with-kata.md) for the configuration and verification steps.

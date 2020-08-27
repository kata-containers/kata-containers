- [Host cgroup management](#host-cgroup-management)
  - [Introduction](#introduction)
  - [`SandboxCgroupOnly` enabled](#sandboxcgrouponly-enabled)
    - [What does Kata do in this configuration?](#what-does-kata-do-in-this-configuration)
    - [Why create a Kata-cgroup under the parent cgroup?](#why-create-a-kata-cgroup-under-the-parent-cgroup)
    - [Improvements](#improvements)
  - [`SandboxCgroupOnly` disabled (default, legacy)](#sandboxcgrouponly-disabled-default-legacy)
    - [What does this method do?](#what-does-this-method-do)
      - [Impact](#impact)
- [Supported cgroups](#supported-cgroups)
  - [Cgroups V1](#cgroups-v1)
  - [Cgroups V2](#cgroups-v2)
    - [Distro Support](#distro-support)
- [Summary](#summary)

# Host cgroup management

## Introduction

In Kata Containers, workloads run in a virtual machine that is managed by a virtual
machine monitor (VMM) running on the host. As a result, Kata Containers run over two layers of cgroups. The
first layer is in the guest where the workload is placed, while the second layer is on the host where the
VMM and associated threads are running.

The OCI [runtime specification][linux-config] provides guidance on where the container cgroups should be placed:

  > [`cgroupsPath`][cgroupspath]: (string, OPTIONAL) path to the cgroups. It can be used to either control the cgroups
  > hierarchy for containers or to run a new process in an existing container

cgroups are hierarchical, and this can be seen with the following pod example:

- Pod 1: `cgroupsPath=/kubepods/pod1`
  - Container 1:
`cgroupsPath=/kubepods/pod1/container1`
  - Container 2:
`cgroupsPath=/kubepods/pod1/container2`

- Pod 2: `cgroupsPath=/kubepods/pod2`
  - Container 1:
`cgroupsPath=/kubepods/pod2/container2`
  - Container 2:
`cgroupsPath=/kubepods/pod2/container2`

Depending on the upper-level orchestrator, the cgroup under which the pod is placed is
managed by the orchestrator. In the case of Kubernetes, the pod-cgroup is created by Kubelet,
while the container cgroups are to be handled by the runtime. Kubelet will size the pod-cgroup
based on the container resource requirements.

Kata Containers introduces a non-negligible overhead for running a sandbox (pod). Based on this, two scenarios are possible:
 1) The upper-layer orchestrator takes the overhead of running a sandbox into account when sizing the pod-cgroup, or
 2) Kata Containers do not fully constrain the VMM and associated processes, instead placing a subset of them outside of the pod-cgroup.

Kata Containers provides two options for how cgroups are handled on the host. Selection of these options is done through
the `SandboxCgroupOnly` flag within the Kata Containers [configuration](../../src/runtime/README.md#configuration)
file.

## `SandboxCgroupOnly` enabled

With `SandboxCgroupOnly` enabled, it is expected that the parent cgroup is sized to take the overhead of running
a sandbox into account. This is ideal, as all the applicable Kata Containers components can be placed within the
given cgroup-path.

In the context of Kubernetes, Kubelet will size the pod-cgroup to take the overhead of running a Kata-based sandbox
into account. This will be feasible in the 1.16 Kubernetes release through the `PodOverhead` feature.

```
+----------------------------------------------------------+
|    +---------------------------------------------------+ |
|    |   +---------------------------------------------+ | |
|    |   |   +--------------------------------------+  | | |
|    |   |   | kata-shimv2, VMM and threads:        |  | | |
|    |   |   |  (VMM, IO-threads, vCPU threads, etc)|  | | |
|    |   |   |                                      |  | | |
|    |   |   | kata_<sandbox-id>                    |  | | |
|    |   |   +--------------------------------------+  | | |
|    |   |                                             | | |
|    |   |Pod 1                                        | | |
|    |   +---------------------------------------------+ | |
|    |                                                   | |
|    |   +---------------------------------------------+ | |
|    |   |   +--------------------------------------+  | | |
|    |   |   | kata-shimv2, VMM and threads:        |  | | |
|    |   |   |  (VMM, IO-threads, vCPU threads, etc)|  | | |
|    |   |   |                                      |  | | |
|    |   |   | kata_<sandbox-id>                    |  | | |
|    |   |   +--------------------------------------+  | | |
|    |   |Pod 2                                        | | |
|    |   +---------------------------------------------+ | |
|    |kubepods                                           | |
|    +---------------------------------------------------+ |
|                                                          |
|Node                                                      |
+----------------------------------------------------------+
```

### What does Kata do in this configuration?
1. Given a `PodSandbox` container creation, let:

   ```
   podCgroup=Parent(container.CgroupsPath)
   KataSandboxCgroup=<podCgroup>/kata_<PodSandboxID>
   ```

2. Create the cgroup, `KataSandboxCgroup`

3. Join the `KataSandboxCgroup`

Any process created by the runtime will be created in `KataSandboxCgroup`.
The runtime will limit the cgroup in the host only if the sandbox doesn't have a
container type annotation, but the caller is free to set the proper limits for the `podCgroup`.

In the example above the pod cgroups are `/kubepods/pod1` and `/kubepods/pod2`.
Kata creates the unrestricted sandbox cgroup under the pod cgroup.

### Why create a Kata-cgroup under the parent cgroup?

`Docker` does not have a notion of pods, and will not create a cgroup directory
to place a particular container in (i.e., all containers would be in a path like
`/docker/container-id`. To simplify the implementation and continue to support `Docker`,
Kata Containers creates the sandbox-cgroup, in the case of Kubernetes, or a container cgroup, in the case
of docker.

### Improvements

- Get statistics about pod resources

If the Kata caller wants to know the resource usage on the host it can get
statistics from the pod cgroup. All cgroups stats in the hierarchy will include
the Kata overhead. This gives the possibility of gathering usage-statics at the
pod level and the container level.

- Better host resource isolation

Because the Kata runtime will place all the Kata processes in the pod cgroup,
the resource limits that the caller applies to the pod cgroup will affect all
processes that belong to the Kata sandbox in the host. This will improve the
isolation in the host preventing Kata to become a noisy neighbor.

## `SandboxCgroupOnly` disabled (default, legacy)

If the cgroup provided to Kata is not sized appropriately, instability will be
introduced when fully constraining Kata components, and the user-workload will
see a subset of resources that were requested. Based on this, the default
handling for Kata Containers is to not fully constrain the VMM and Kata
components on the host.

```
+----------------------------------------------------------+
|    +---------------------------------------------------+ |
|    |   +---------------------------------------------+ | |
|    |   |   +--------------------------------------+  | | |
|    |   |   |Container 1       |-|Container 2      |  | | |
|    |   |   |                  |-|                 |  | | |
|    |   |   | Shim+container1  |-| Shim+container2 |  | | |
|    |   |   +--------------------------------------+  | | |
|    |   |                                             | | |
|    |   |Pod 1                                        | | |
|    |   +---------------------------------------------+ | |
|    |                                                   | |
|    |   +---------------------------------------------+ | |
|    |   |   +--------------------------------------+  | | |
|    |   |   |Container 1       |-|Container 2      |  | | |
|    |   |   |                  |-|                 |  | | |
|    |   |   | Shim+container1  |-| Shim+container2 |  | | |
|    |   |   +--------------------------------------+  | | |
|    |   |                                             | | |
|    |   |Pod 2                                        | | |
|    |   +---------------------------------------------+ | |
|    |kubepods                                           | |
|    +---------------------------------------------------+ |
|    +---------------------------------------------------+ |
|    |  Hypervisor                                       | |
|    |Kata                                               | |
|    +---------------------------------------------------+ |
|                                                          |
|Node                                                      |
+----------------------------------------------------------+

```

### What does this method do?

1. Given a container creation let `containerCgroupHost=container.CgroupsPath`
1. Rename `containerCgroupHost` path to add `kata_`
1. Let `PodCgroupPath=PodSanboxContainerCgroup` where `PodSanboxContainerCgroup` is the cgroup of a container of type `PodSandbox`
1. Limit the `PodCgroupPath` with the sum of all the container limits in the Sandbox
1. Move only vCPU threads of hypervisor to `PodCgroupPath`
1. Per each container, move its `kata-shim` to its own `containerCgroupHost`
1. Move hypervisor and applicable threads to memory cgroup `/kata`

_Note_: the Kata Containers runtime will not add all the hypervisor threads to
the cgroup path requested, only vCPUs. These threads are run unconstrained.

This mitigates the risk of the VMM and other threads receiving an out of memory scenario (`OOM`).


#### Impact

If resources are reserved at a system level to account for the overheads of
running sandbox containers, this configuration can be utilized with adequate
stability. In this scenario, non-negligible amounts of CPU and memory will be
utilized unaccounted for on the host.

[linux-config]: https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md
[cgroupspath]: https://github.com/opencontainers/runtime-spec/blob/master/config-linux.md#cgroups-path

# Supported cgroups

Kata Containers supports cgroups `v1` and `v2`. In the following sections each cgroup is
described briefly and what changes are needed in Kata Containers to support it.

## Cgroups V1

`Cgroups V1` are under a [`tmpfs`][1] filesystem mounted at `/sys/fs/cgroup`, where each cgroup is
mounted under a separate cgroup filesystem. A `Cgroups v1` hierarchy may look like the following
diagram:

```
/sys/fs/cgroup/
├── blkio
│   ├── cgroup.procs
│   └── tasks
├── cpu -> cpu,cpuacct
├── cpuacct -> cpu,cpuacct
├── cpu,cpuacct
│   ├── cgroup.procs
│   └── tasks
├── cpuset
│   ├── cgroup.procs
│   └── tasks
├── devices
│   ├── cgroup.procs
│   └── tasks
├── freezer
│   ├── cgroup.procs
│   └── tasks
├── hugetlb
│   ├── cgroup.procs
│   └── tasks
├── memory
│   ├── cgroup.procs
│   └── tasks
├── net_cls -> net_cls,net_prio
├── net_cls,net_prio
│   ├── cgroup.procs
│   └── tasks
├── net_prio -> net_cls,net_prio
├── perf_event
│   ├── cgroup.procs
│   └── tasks
├── pids
│   ├── cgroup.procs
│   └── tasks
└── systemd
    ├── cgroup.procs
    └── tasks
```

A process can join a cgroup by writing its process id (`pid`) to `cgroup.procs` file,
or join a cgroup partially by writing the task (thread) id (`tid`) to the `tasks` file.

Kata Containers supports `v1` by default and no change in the configuration file is needed.
To know more about `cgroups v1`, see [cgroupsv1(7)][2].

## Cgroups V2

`Cgroups v2` are also known as unified cgroups, unlike `cgroups v1`, the cgroups are
mounted under the same cgroup filesystem. A `Cgroups v2` hierarchy may look like the following
diagram:

```
/sys/fs/cgroup/system.slice
├── cgroup.controllers
├── cgroup.events
├── cgroup.freeze
├── cgroup.max.depth
├── cgroup.max.descendants
├── cgroup.procs
├── cgroup.stat
├── cgroup.subtree_control
├── cgroup.threads
├── cgroup.type
├── cpu.max
├── cpu.pressure
├── cpu.stat
├── cpu.weight
├── cpu.weight.nice
├── io.bfq.weight
├── io.latency
├── io.max
├── io.pressure
├── io.stat
├── memory.current
├── memory.events
├── memory.events.local
├── memory.high
├── memory.low
├── memory.max
├── memory.min
├── memory.oom.group
├── memory.pressure
├── memory.stat
├── memory.swap.current
├── memory.swap.events
├── memory.swap.max
├── pids.current
├── pids.events
└── pids.max
```

Same as `cgroups v1`, a process can join the cgroup by writing its process id (`pid`) to
`cgroup.procs` file, or join a cgroup partially by writing the task (thread) id (`tid`) to
`cgroup.threads` file.

For backwards compatibility Kata Containers defaults to supporting cgroups v1 by default.
To change this to `v2`, set `sandbox_cgroup_only=true` in the `configuration.toml` file.
To know more about `cgroups v2`, see [cgroupsv2(7)][3].

### Distro Support

Many Linux distributions do not yet support `cgroups v2`, as it is quite a recent addition.
For more information about the status of this feature see [issue #2494][4].

# Summary

| cgroup option | default? | status | pros | cons | cgroups
|-|-|-|-|-|-|
| `SandboxCgroupOnly=false` | yes | legacy | Easiest to make Kata work | Unaccounted for memory and resource utilization | v1
| `SandboxCgroupOnly=true` | no | recommended | Complete tracking of Kata memory and CPU utilization. In Kubernetes, the Kubelet can fully constrain Kata via the pod cgroup | Requires upper layer orchestrator which sizes sandbox cgroup appropriately | v1, v2


[1]: http://man7.org/linux/man-pages/man5/tmpfs.5.html
[2]: http://man7.org/linux/man-pages/man7/cgroups.7.html#CGROUPS_VERSION_1
[3]: http://man7.org/linux/man-pages/man7/cgroups.7.html#CGROUPS_VERSION_2
[4]: https://github.com/kata-containers/runtime/issues/2494

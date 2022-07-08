# Host cgroup management

## Introduction

In Kata Containers, workloads run in a virtual machine that is managed by a virtual
machine monitor (VMM) running on the host. As a result, Kata Containers run over two layers of cgroups. The
first layer is in the guest where the workload is placed, while the second layer is on the host where the
VMM and associated threads are running.

The OCI [runtime specification][linux-config] provides guidance on where the container cgroups should be placed:

  > [`cgroupsPath`][cgroupspath]: (string, OPTIONAL) path to the cgroups. It can be used to either control the cgroups
  > hierarchy for containers or to run a new process in an existing container

The cgroups are hierarchical, and this can be seen with the following pod example:

- Pod 1: `cgroupsPath=/kubepods/pod1`
  - Container 1: `cgroupsPath=/kubepods/pod1/container1`
  - Container 2: `cgroupsPath=/kubepods/pod1/container2`

- Pod 2: `cgroupsPath=/kubepods/pod2`
  - Container 1: `cgroupsPath=/kubepods/pod2/container1`
  - Container 2: `cgroupsPath=/kubepods/pod2/container2`

Depending on the upper-level orchestration layers, the cgroup under which the pod is placed is
managed by the orchestrator or not. In the case of Kubernetes, the pod cgroup is created by Kubelet,
while the container cgroups are to be handled by the runtime.
Kubelet will size the pod cgroup based on the container resource requirements, to which it may add
a configured set of [pod resource overheads](https://kubernetes.io/docs/concepts/scheduling-eviction/pod-overhead/).

Kata Containers introduces a non-negligible resource overhead for running a sandbox (pod). Typically, the Kata shim,
through its underlying VMM invocation, will create many additional threads compared to process based container runtimes:
the para-virtualized I/O back-ends, the VMM instance or even the Kata shim process, all of those host processes consume
memory and CPU time not directly tied to the container workload, and introduces a sandbox resource overhead.
In order for a Kata workload to run without significant performance degradation, its sandbox overhead must be
provisioned accordingly. Two scenarios are possible:

 1) The upper-layer orchestrator takes the overhead of running a sandbox into account when sizing the pod cgroup.
    For example, Kubernetes [`PodOverhead`](https://kubernetes.io/docs/concepts/scheduling-eviction/pod-overhead/)
	feature lets the orchestrator add a configured sandbox overhead to the sum of all its containers resources. In
	that case, the pod sandbox is properly sized and all Kata created processes will run under the pod cgroup
	defined constraints and limits.
 2) The upper-layer orchestrator does **not** take the sandbox overhead into account and the pod cgroup is not
	sized to properly run all Kata created processes. With that scenario, attaching all the Kata processes to the sandbox
	cgroup may lead to non-negligible workload performance degradations. As a consequence, Kata Containers will move
	all processes but the vCPU threads into a dedicated overhead cgroup under `/kata_overhead`. The Kata runtime will
	not apply any constraints or limits to that cgroup, it is up to the infrastructure owner to optionally set it up.

Those 2 scenarios are not dynamically detected by the Kata Containers runtime implementation, and thus the
infrastructure owner must configure the runtime according to how the upper-layer orchestrator creates and sizes the
pod cgroup. That configuration selection is done through the `sandbox_cgroup_only` flag within the Kata Containers
[configuration](../../src/runtime/README.md#configuration) file.

## `sandbox_cgroup_only = true`

Setting `sandbox_cgroup_only` to `true` from the Kata Containers configuration file means that the pod cgroup is
properly sized and takes the pod overhead into account. This is ideal, as all the applicable Kata Containers processes
can simply be placed within the given cgroup path.

In the context of Kubernetes, Kubelet can size the pod cgroup to take the overhead of running a Kata-based sandbox
into account. This has been supported since the 1.16 Kubernetes release, through the
[`PodOverhead`](https://kubernetes.io/docs/concepts/scheduling-eviction/pod-overhead/) feature.

```
┌─────────────────────────────────────────┐
│                                         │
│  ┌──────────────────────────────────┐   │
│  │                                  │   │
│  │ ┌─────────────────────────────┐  │   │
│  │ │                             │  │   │
│  │ │ ┌─────────────────────┐     │  │   │
│  │ │ │ vCPU threads        │     │  │   │
│  │ │ │ I/O threads         │     │  │   │
│  │ │ │ VMM                 │     │  │   │
│  │ │ │ Kata Shim           │     │  │   │
│  │ │ │                     │     │  │   │
│  │ │ │ /kata_<sandbox_id>  │     │  │   │
│  │ │ └─────────────────────┘     │  │   │
│  │ │Pod 1                        │  │   │
│  │ └─────────────────────────────┘  │   │
│  │                                  │   │
│  │ ┌─────────────────────────────┐  │   │
│  │ │                             │  │   │
│  │ │ ┌─────────────────────┐     │  │   │
│  │ │ │ vCPU threads        │     │  │   │
│  │ │ │ I/O threads         │     │  │   │
│  │ │ │ VMM                 │     │  │   │
│  │ │ │ Kata Shim           │     │  │   │
│  │ │ │                     │     │  │   │
│  │ │ │ /kata_<sandbox_id>  │     │  │   │
│  │ │ └─────────────────────┘     │  │   │
│  │ │Pod 2                        │  │   │
│  │ └─────────────────────────────┘  │   │
│  │                                  │   │
│  │/kubepods                         │   │
│  └──────────────────────────────────┘   │
│                                         │
│ Node                                    │
└─────────────────────────────────────────┘
```

### Implementation details

When `sandbox_cgroup_only` is enabled, the Kata shim will create a per pod
sub-cgroup under the pod's dedicated cgroup. For example, in the Kubernetes context,
it will create a `/kata_<PodSandboxID>` under the `/kubepods` cgroup hierarchy.
On a typical cgroup v1 hierarchy mounted under `/sys/fs/cgroup/`, the memory cgroup
subsystem for a pod with sandbox ID `12345678` would live under
`/sys/fs/cgroup/memory/kubepods/kata_12345678`.

In most cases, the `/kata_<PodSandboxID>` created cgroup is unrestricted and inherits and shares all
constraints and limits from the parent cgroup (`/kubepods` in the Kubernetes case). The exception is
for the `cpuset` and `devices` cgroup subsystems, which are managed by the Kata shim.

After creating the `/kata_<PodSandboxID>` cgroup, the Kata Containers shim will move itself to it, **before** starting
the virtual machine. As a consequence all processes subsequently created by the Kata Containers shim (the VMM itself, and
all vCPU and I/O related threads) will be created in the `/kata_<PodSandboxID>` cgroup.

### Why create a kata-cgroup under the parent cgroup?

And why not directly adding the per sandbox shim directly to the pod cgroup (e.g. 
`/kubepods` in the Kubernetes context)?

The Kata Containers shim implementation creates a per-sandbox cgroup
(`/kata_<PodSandboxID>`) to support the `Docker` use case. Although `Docker` does not
have a notion of pods, Kata Containers still creates a sandbox to support the pod-less,
single container use case that `Docker` implements. Since `Docker` does create any
cgroup hierarchy to place a container into, it would be very complex for Kata to map
a particular container to its sandbox without placing it under a `/kata_<containerID>>`
sub-cgroup first.

### Advantages

Keeping all Kata Containers processes under a properly sized pod cgroup is ideal
and makes for a simpler Kata Containers implementation. It also helps with gathering
accurate statistics and preventing Kata workloads from being noisy neighbors.

#### Pod resources statistics

If the Kata caller wants to know the resource usage on the host it can get
statistics from the pod cgroup. All cgroups stats in the hierarchy will include
the Kata overhead. This gives the possibility of gathering usage-statics at the
pod level and the container level.

#### Better host resource isolation

Because the Kata runtime will place all the Kata processes in the pod cgroup,
the resource limits that the caller applies to the pod cgroup will affect all
processes that belong to the Kata sandbox in the host. This will improve the
isolation in the host preventing Kata to become a noisy neighbor.

## `sandbox_cgroup_only = false` (Default setting)

If the cgroup provided to Kata is not sized appropriately, Kata components will
consume resources that the actual container workloads expect to see and use.
This can cause instability and performance degradations.

To avoid that situation, Kata Containers creates an unconstrained overhead
cgroup and moves all non workload related processes (Anything but the virtual CPU
threads) to it. The name of this overhead cgroup is `/kata_overhead` and a per
sandbox sub cgroup will be created under it for each sandbox Kata Containers creates.

Kata Containers does not add any constraints or limitations on the overhead cgroup. It is up to the infrastructure
owner to either:

- Provision nodes with a pre-sized `/kata_overhead` cgroup. Kata Containers will
  load that existing cgroup and move all non workload related processes to it.
- Let Kata Containers create the `/kata_overhead` cgroup, leave it
  unconstrained or resize it a-posteriori.


```
┌────────────────────────────────────────────────────────────────────┐
│                                                                    │
│  ┌─────────────────────────────┐    ┌───────────────────────────┐  │
│  │                             │    │                           │  │
│  │   ┌─────────────────────────┼────┼─────────────────────────┐ │  │
│  │   │                         │    │                         │ │  │
│  │   │ ┌─────────────────────┐ │    │ ┌─────────────────────┐ │ │  │
│  │   │ │  vCPU threads       │ │    │ │  VMM                │ │ │  │
│  │   │ │                     │ │    │ │  I/O threads        │ │ │  │
│  │   │ │                     │ │    │ │  Kata Shim          │ │ │  │
│  │   │ │                     │ │    │ │                     │ │ │  │
│  │   │ │ /kata_<sandbox_id>  │ │    │ │ /<sandbox_id>       │ │ │  │
│  │   │ └─────────────────────┘ │    │ └─────────────────────┘ │ │  │
│  │   │                         │    │                         │ │  │
│  │   │  Pod 1                  │    │                         │ │  │
│  │   └─────────────────────────┼────┼─────────────────────────┘ │  │
│  │                             │    │                           │  │
│  │                             │    │                           │  │
│  │   ┌─────────────────────────┼────┼─────────────────────────┐ │  │
│  │   │                         │    │                         │ │  │
│  │   │ ┌─────────────────────┐ │    │ ┌─────────────────────┐ │ │  │
│  │   │ │  vCPU threads       │ │    │ │  VMM                │ │ │  │
│  │   │ │                     │ │    │ │  I/O threads        │ │ │  │
│  │   │ │                     │ │    │ │  Kata Shim          │ │ │  │
│  │   │ │                     │ │    │ │                     │ │ │  │
│  │   │ │ /kata_<sandbox_id>  │ │    │ │ /<sandbox_id>       │ │ │  │
│  │   │ └─────────────────────┘ │    │ └─────────────────────┘ │ │  │
│  │   │                         │    │                         │ │  │
│  │   │  Pod 2                  │    │                         │ │  │
│  │   └─────────────────────────┼────┼─────────────────────────┘ │  │
│  │                             │    │                           │  │
│  │ /kubepods                   │    │ /kata_overhead            │  │
│  └─────────────────────────────┘    └───────────────────────────┘  │
│                                                                    │
│                                                                    │
│ Node                                                               │
└────────────────────────────────────────────────────────────────────┘

```

### Implementation Details

When `sandbox_cgroup_only` is disabled, the Kata Containers shim will create a per pod
sub-cgroup under the pods dedicated cgroup, and another one under the overhead cgroup.
For example, in the Kubernetes context, it will create a `/kata_<PodSandboxID>` under
the `/kubepods` cgroup hierarchy, and a `/<PodSandboxID>` under the `/kata_overhead` one.

On a typical cgroup v1 hierarchy mounted under `/sys/fs/cgroup/`, for a pod which sandbox
ID is `12345678`, create with `sandbox_cgroup_only` disabled, the 2 memory subsystems
for the sandbox cgroup and the overhead cgroup would respectively live under 
`/sys/fs/cgroup/memory/kubepods/kata_12345678` and `/sys/fs/cgroup/memory/kata_overhead/12345678`.

Unlike when `sandbox_cgroup_only` is enabled, the Kata Containers shim will move itself
to the overhead cgroup first, and then move the vCPU threads to the sandbox cgroup as
they're created. All Kata processes and threads will run under the overhead cgroup except for
the vCPU threads. 

With `sandbox_cgroup_only` disabled, Kata Containers assumes the pod cgroup is only sized
to accommodate for the actual container workloads processes. For Kata, this maps
to the VMM created virtual CPU threads and so they are the only ones running under the pod
cgroup. This mitigates the risk of the VMM, the Kata shim and the I/O threads going through
a catastrophic out of memory scenario (`OOM`).

#### Pros and Cons

Running all non vCPU threads under an unconstrained overhead cgroup could lead to workloads
potentially consuming a large amount of host resources.

On the other hand, running all non vCPU threads under a dedicated overhead cgroup can provide
accurate metrics on the actual Kata Container pod overhead, allowing for tuning the overhead
cgroup size and constraints accordingly.

[linux-config]: https://github.com/opencontainers/runtime-spec/blob/main/config-linux.md
[cgroupspath]: https://github.com/opencontainers/runtime-spec/blob/main/config-linux.md#cgroups-path

# Supported cgroups

Kata Containers currently supports cgroups `v1` and `v2`. 

In the following sections each cgroup is described briefly.

## cgroups v1

`cgroups v1` are under a [`tmpfs`][1] filesystem mounted at `/sys/fs/cgroup`, where each cgroup is
mounted under a separate cgroup filesystem. A `cgroups v1` hierarchy may look like the following
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

To know more about `cgroups v1`, see [cgroupsv1(7)][2].

## cgroups v2

`cgroups v2` are also known as unified cgroups, unlike `cgroups v1`, the cgroups are
mounted under the same cgroup filesystem. A `cgroups v2` hierarchy may look like the following
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

### Distro Support

Many Linux distributions do not yet support `cgroups v2`, as it is quite a recent addition.
For more information about the status of this feature see [issue #2494][4].


[1]: http://man7.org/linux/man-pages/man5/tmpfs.5.html
[2]: http://man7.org/linux/man-pages/man7/cgroups.7.html#CGROUPS_VERSION_1
[3]: http://man7.org/linux/man-pages/man7/cgroups.7.html#CGROUPS_VERSION_2
[4]: https://github.com/kata-containers/runtime/issues/2494

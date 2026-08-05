# Kata Containers with virtio-fs

## Introduction

Container deployments utilize explicit or implicit file sharing between host filesystem and containers. From a trust perspective, avoiding a shared file-system between the trusted host and untrusted container is recommended. This is not always feasible. In Kata Containers, block-based volumes are preferred as they allow usage of either device pass through or `virtio-blk` for access within the virtual machine.

As of the 2.0 release of Kata Containers, [virtio-fs](https://virtio-fs.gitlab.io/) is the default filesystem sharing mechanism. In Kata Containers, virtio-fs is used to share container volumes, Secrets, ConfigMaps, configuration files (`hostname`, `hosts`, `resolv.conf`), and the container rootfs on the host with the guest. `virtio-fs` provides significant performance and POSIX compliance improvements over `9pfs`.

`virtio-fs` support works out of the box for `cloud-hypervisor` and `qemu` when Kata Containers is deployed using `kata-deploy`. See the [`kata-deploy` documentation](../../tools/packaging/kata-deploy/helm-chart/README.md) to learn how to deploy Kata Containers in Kubernetes.

## Host shared memory (`/dev/shm`) sizing

`virtio-fs` is a vhost-user device: the `virtiofsd` daemon runs as a regular process on the host and serves file contents by reading and writing guest memory directly. For this to work, the guest RAM must be allocated as shared memory. With QEMU, Kata Containers backs the guest memory with a file-backed, `share=on` memory object placed on `/dev/shm`. For every running Kata Pod, the memory object on `/dev/shm` is sized to the VM memory (`default_memory` plus any hot-plugged memory); the actual `tmpfs` consumption grows as the guest touches its pages, up to that size.

!!! note
    This section applies to QEMU without hugepages. With `enable_hugepages` set, QEMU places the guest memory on `/dev/hugepages` instead, and Cloud Hypervisor uses `memfd`-based shared memory. Neither is limited by the size of the `/dev/shm` mount.

`/dev/shm` is a `tmpfs` mount and its default size is half of the physical memory. This caps the aggregate guest memory of all Kata Pods on a node at 50% of the node's RAM. Note that this is a limit on the memory that can be handed to Kata Pods, not a fixed Pod count: a 256 GiB node has a 128 GiB `/dev/shm` by default, which, at the default `default_memory` of 2048 MiB, is exhausted after roughly 64 Kata Pods, even though the node still has plenty of free memory.

!!! warning
    `tmpfs` pages are allocated on first touch. A VM whose boot memory does not fit into `/dev/shm` fails to start; worse, an already-running VM that touches pages beyond the mount limit receives a `SIGBUS` and is killed. The kubelet cannot anticipate either failure because it does not track `/dev/shm` capacity.

The size of `/dev/shm` can be increased at runtime:

```bash
$ sudo mount -o remount,size=${desired_shm_size} /dev/shm
```

The remount does not survive a reboot. To make the size persistent, update the existing `/dev/shm` entry in `/etc/fstab`, or add one if your distribution does not define it. Keep the existing mount options and only set `size=`:

```text title="/etc/fstab"
none /dev/shm tmpfs defaults,nosuid,nodev,size=248G 0 0
```

### Aligning the kubelet memory reservations

Kubernetes schedules Pods against the node's *Allocatable* memory:

```text
allocatable = capacity - kubeReserved - systemReserved - evictionHard["memory.available"]
```

The kubelet does not know the size of `/dev/shm`, so the two must be kept consistent by configuration:

* If `/dev/shm` is **smaller** than Allocatable (the default), the kubelet keeps scheduling Kata Pods that can never get their memory: the scheduler still sees free memory while VM creation fails or running VMs receive a `SIGBUS`.
* If `/dev/shm` is **larger** than Allocatable (for example sized to 100% of RAM without reserving anything for the system), the aggregate VM memory can eat into the memory the kubelet assumes is reserved for itself and the system daemons. Memory requests are then no longer backed by physical memory and the node can be driven into an out-of-memory situation.

Reserve memory for the system and the kubelet explicitly, and size `/dev/shm` to what is left, which is the memory you actually intend to hand out to Kata Pods:

```yaml title="KubeletConfiguration (fragment)"
systemReserved:
  memory: "4Gi"
kubeReserved:
  memory: "2Gi"
evictionHard:
  memory.available: "2Gi"
```

The two reservations cover different sets of daemons:

* `kubeReserved` accommodates the Kubernetes system daemons: the kubelet itself and the container runtime, for example `containerd`.
* `systemReserved` accommodates the operating system daemons running outside of Kubernetes: `systemd`, `journald`, `sshd`, `udev`, monitoring and logging agents, and similar.

!!! note
    By default the kubelet only enforces the `pods` boundary (`enforceNodeAllocatable: ["pods"]`). The reservations reduce Allocatable so that the scheduler leaves headroom for the daemons; they do not constrain the daemons themselves.

The per-Pod Kata host processes (the `containerd-shim-kata-v2` process, `virtiofsd`, and the non-vCPU threads of the VMM) do not need to be part of these static reservations: with the default `sandbox_cgroup_only = false` they run in a dedicated, unconstrained `/kata_overhead` cgroup, and their budget is the per-Pod Pod Overhead described in the next section, which scales with the number of Kata Pods on the node. See the [host cgroups design document](../design/host-cgroups.md) for details.

For a 256 GiB node this yields `256 - 4 - 2 - 2 = 248` GiB of Allocatable memory, which is the upper bound for the size of `/dev/shm`:

```bash
$ sudo mount -o remount,size=248G /dev/shm
```

In practice every node operates in a mixed mode: even a node dedicated to Kata workloads runs `runc` Pods for the Kubernetes infrastructure, such as the CNI and CSI node plugins, `kube-proxy`, monitoring and logging DaemonSets, and `kata-deploy` itself. These infrastructure Pods use regular anonymous memory and do not consume `/dev/shm`, but their requests count against the same Allocatable. For workload Pods, on the other hand, mixing `runc` and Kata on the same node is not recommended: run the workloads as Kata Pods so that the isolation guarantees are uniform and the `/dev/shm` sizing stays predictable. Suggested sizing:

* The node's Allocatable memory is the upper bound: `/dev/shm` never needs to be larger than that.
* Subtract the memory requests of the infrastructure `runc` Pods (the DaemonSets above) from Allocatable and size `/dev/shm` to the remainder. This is the aggregate VM memory available to Kata Pods.

### Pod memory accounting

The vCPU threads of the hypervisor run inside the Pod's sandbox cgroup and `tmpfs` pages are charged to the cgroup that first touches them, so the bulk of the guest memory is charged against the Pod's memory cgroup and is visible to Pod-level limits and kubelet eviction. Pages first touched by `virtiofsd` or by the non-vCPU threads of the VMM are charged to the unconstrained `/kata_overhead` cgroup instead when `sandbox_cgroup_only = false`; this share is not subject to per-Pod limits and is budgeted at scheduling time through the Pod Overhead. The fixed virtualization overhead (the shim, `virtiofsd`, and the non-vCPU parts of the VMM) is declared in the `RuntimeClass` via [Pod Overhead](https://kubernetes.io/docs/concepts/scheduling-eviction/pod-overhead/), which the scheduler adds to the Pod's resource requests. The RuntimeClasses installed by `kata-deploy` declare this overhead by default, with values that differ per shim, for example 320Mi for `kata-qemu`, 130Mi for `kata-clh`, and substantially more for the confidential computing and GPU variants:

```yaml title="RuntimeClass kata-qemu (fragment)"
overhead:
  podFixed:
    memory: "320Mi"
    cpu: "250m"
```

With `/dev/shm` sized to Allocatable, the reservations enforced by the kubelet, and the Pod Overhead declared, the memory a Kata Pod consumes through `/dev/shm` is fully visible to Kubernetes: memory requests remain guaranteed and the node cannot be overcommitted by Kata VMs.

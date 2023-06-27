# Virtual machine vCPU sizing in Kata Containers 3.0

> Preview: 
> [Kubernetes(since 1.23)][1] and [Containerd(since 1.6.0-beta4)][2] will help calculate `Sandbox Size` info and pass it to Kata Containers through annotations.
> In order to adapt to this beneficial change and be compatible with the past, we have implemented the new vCPUs handling way in `runtime-rs`, which is slightly different from the original `runtime-go`'s design.

## When do we need to handle vCPUs size?
vCPUs sizing should be determined by the container workloads. So throughout the life cycle of Kata Containers, there are several points in time when we need to think about how many vCPUs should be at the time. Mainly including the time points of `CreateVM`, `CreateContainer`, `UpdateContainer`, and `DeleteContainer`.
* `CreateVM`: When creating a sandbox, we need to know how many vCPUs to start the VM with.
* `CreateContainer`: When creating a new container in the VM, we may need to hot-plug the vCPUs according to the requirements in container's spec.
* `UpdateContainer`: When receiving the `UpdateContainer` request, we may need to update the vCPU resources according to the new requirements of the container.
* `DeleteContainer`: When a container is removed from the VM, we may need to hot-unplug the vCPUs to reclaim the vCPU resources introduced by the container.

## On what basis do we calculate the number of vCPUs?
When Kata calculate the number of vCPUs, We have three data sources, the `default_vcpus` and `default_maxvcpus` specified in the configuration file (named `TomlConfig` later in the doc), the `io.kubernetes.cri.sandbox-cpu-quota` and `io.kubernetes.cri.sandbox-cpu-period` annotations passed by the upper layer runtime, and the corresponding CPU resource part in the container's spec for the container when `CreateContainer`/`UpdateContainer`/`DeleteContainer` is requested.

Our understanding and priority of these resources are as follows, which will affect how we calculate the number of vCPUs later.

* From `TomlConfig`:
  * `default_vcpus`: default number of vCPUs when starting a VM.
  * `default_maxvcpus`: maximum number of vCPUs.
* From `Annotation`:
  * `InitialSize`: we call the size of the resource passed from the annotations as `InitialSize`. Kubernetes will calculate the sandbox size according to the Pod's statement, which is the `InitialSize` here. This size should be the size we want to prioritize. 
* From `Container Spec`:
  * The amount of CPU resources that the Container wants to use will be declared through the spec. Including the aforementioned annotations, we mainly consider `cpu quota` and `cpuset` when calculating the number of vCPUs.
  * `cpu quota`: `cpu quota` is the most common way to declare the amount of CPU resources. The number of vCPUs introduced by `cpu quota` declared in a container's spec is: `vCPUs = ceiling( quota / period )`.
  * `cpuset`: `cpuset` is often used to bind the CPUs that tasks can run on. The number of vCPUs may introduced by `cpuset` declared in a container's spec is the number of CPUs specified in the set that do not overlap with other containers.


## How to calculate and adjust the vCPUs size:
There are two types of vCPUs that we need to consider, one is the number of vCPUs when starting the VM (named `Boot Size` in the doc). The second is the number of vCPUs when `CreateContainer`/`UpdateContainer`/`DeleteContainer` request is received (`Real-time Size` in the doc).

### `Boot Size`
The main considerations are `InitialSize` and `default_vcpus`. There are the following principles:
`InitialSize` has priority over `default_vcpus` declared in `TomlConfig`.
1. When there is such an annotation statement, the originally `default_vcpus` will be modified to the number of vCPUs in the `InitialSize` as the `Boot Size`. (Because not all runtimes support this annotation for the time being, we still keep the `default_cpus` in `TomlConfig`.)
2. When the specs of all containers are aggregated for sandbox size calculation, the method is consistent with the calculation method of `InitialSize` here.

### `Real-time Size`
When we receive an OCI request, it may be for a single container. But what we have to consider is the number of vCPUs for the entire VM. So we will maintain a list. Every time there is a demand for adjustment, the entire list will be traversed to calculate a value for the number of vCPUs. In addition, there are the following principles:
1. Do not cut computing power and try to keep the number of vCPUs specified by `InitialSize`.
   * So the number of vCPUs after will not be less than the `Boot Size`.
2. `cpu quota` takes precedence over `cpuset` and the setting history are took into account.
   * We think quota describes the CPU time slice that a cgroup can use, and `cpuset` describes the actual CPU number that a cgroup can use. Quota can better describe the size of the CPU time slice that a cgroup actually wants to use. The `cpuset` only describes which CPUs the cgroup can use, but the cgroup can use the specified CPU but consumes a smaller time slice, so the quota takes precedence over the `cpuset`.
   * On the one hand, when both `cpu quota` and `cpuset` are specified, we will calculate the number of vCPUs based on `cpu quota` and ignore `cpuset`. On the other hand, if `cpu quota` was used to control the number of vCPUs in the past, and only `cpuset` was updated during `UpdateContainer`, we will not adjust the number of vCPUs at this time.
3. `StaticSandboxResourceMgmt` controls hotplug.
   * Some VMMs and kernels of some architectures do not support hotplugging. We can accommodate this situation through `StaticSandboxResourceMgmt`. When `StaticSandboxResourceMgmt = true` is set, we don't make any further attempts to update the number of vCPUs after booting.


[1]: https://github.com/kubernetes/kubernetes/pull/104886
[2]: https://github.com/containerd/containerd/pull/6155

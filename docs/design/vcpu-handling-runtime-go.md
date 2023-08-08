# Virtual machine vCPU sizing in Kata Containers

## Default number of virtual CPUs

Before starting a container, the [runtime][4] reads the `default_vcpus` option
from the [configuration file][5] to determine the number of virtual CPUs
(vCPUs) needed to start the virtual machine. By default, `default_vcpus` is
equal to 1 for fast boot time and a small memory footprint per virtual machine.
Be aware that increasing this value negatively impacts the virtual machine's
boot time and memory footprint.
In general, we recommend that you do not edit this variable, unless you know
what are you doing. If your container needs more than one vCPU, use
[Kubernetes `cpu` limits][1] to assign more resources.

*Kubernetes*

```yaml
# ~/cpu-demo.yaml
apiVersion: v1
kind: Pod
metadata:
  name: cpu-demo
  namespace: sandbox
spec:
  containers:
  - name: cpu0
    image: vish/stress
    resources:
      limits:
        cpu: "3"
    args:
    - -cpus
    - "5"
```

```sh
$ sudo -E kubectl create -f ~/cpu-demo.yaml
```

## Virtual CPUs and Kubernetes pods

A Kubernetes pod is a group of one or more containers, with shared storage and
network, and a specification for how to run the containers [[specification][2]].
In Kata Containers this group of containers, which is called a sandbox, runs inside
the same virtual machine. If you do not specify a CPU constraint, the runtime does
not add more vCPUs and the container is not placed inside a CPU cgroup.
Instead, the container uses the number of vCPUs specified by `default_vcpus`
and shares these resources with other containers in the same situation
(without a CPU constraint).

## Container lifecycle

When you create a container with a CPU constraint, the runtime adds the
number of vCPUs required by the container. Similarly, when the container terminates,
the runtime removes these resources.

## Container without CPU constraint

A container without a CPU constraint uses the default number of vCPUs specified
in the configuration file. In the case of Kubernetes pods, containers without a
CPU constraint use and share between them the default number of vCPUs. For
example, if `default_vcpus` is equal to 1 and you have 2 containers without CPU
constraints with each container trying to consume 100% of vCPU, the resources
divide in two parts, 50% of vCPU for each container because your virtual
machine does not have enough resources to satisfy containers needs. If you want
to give access to a greater or lesser portion of vCPUs to a specific container,
use [Kubernetes `cpu` requests][1].

*Kubernetes*

```yaml
# ~/cpu-demo.yaml
apiVersion: v1
kind: Pod
metadata:
  name: cpu-demo
  namespace: sandbox
spec:
  containers:
  - name: cpu0
    image: vish/stress
    resources:
      requests:
        cpu: "0.7"
    args:
    - -cpus
    - "3"
```

```sh
$ sudo -E kubectl create -f ~/cpu-demo.yaml
```

Before running containers without CPU constraint, consider that your containers
are not running alone. Since your containers run inside a virtual machine other
processes use the vCPUs as well (e.g. `systemd` and the Kata Containers
[agent][3]). In general, we recommend setting `default_vcpus` equal to 1 to
allow non-container processes to run on this vCPU and to specify a CPU
constraint for each container.

## Container with CPU constraint

The runtime calculates the number of vCPUs required by a container with CPU
constraints using the following formula: `vCPUs = ceiling( quota / period )`, where
`quota` specifies the number of microseconds per CPU Period that the container is
guaranteed CPU access and `period` specifies the CPU CFS scheduler period of time
in microseconds. The result determines the number of vCPU to hot plug into the
virtual machine. Once the vCPUs have been added, the [agent][3] places the
container inside a CPU cgroup. This placement allows the container to use only
its assigned resources.

## Do not waste resources

If you already know the number of vCPUs needed for each container and pod, or
just want to run them with the same number of vCPUs, you can specify that
number using the `default_vcpus` option in the configuration file, each virtual
machine starts with that number of vCPUs. One limitation of this approach is
that these vCPUs cannot be removed later and you might be wasting
resources. For example, if you set `default_vcpus` to 8 and run only one
container with a CPU constraint of 1 vCPUs, you might be wasting 7 vCPUs since
the virtual machine starts with 8 vCPUs and 1 vCPUs is added and assigned
to the container. Non-container processes might be able to use 8 vCPUs but they
use a maximum 1 vCPU, hence 7 vCPUs might not be used.

## Virtual CPU handling without hotplug

In some cases, the hardware and/or software architecture being utilized does not support
hotplug. For example, Firecracker VMM does not support CPU or memory hotplug. Similarly,
the current Linux Kernel for aarch64 does not support CPU or memory hotplug. To appropriately
size the virtual machine for the workload within the container or pod, we provide a `static_sandbox_resource_mgmt`
flag within the Kata Containers configuration. When this is set, the runtime will:
 - Size the VM based on the workload requirements as well as the `default_vcpus` option specified in the configuration.
 - Not resize the virtual machine after it has been launched.

VM size determination varies depending on the type of container being run, and may not always
be available. If workload sizing information is not available, the virtual machine will be started with the
`default_vcpus`.

In the case of a pod, the initial sandbox container (pause container) typically doesn't contain any resource
information in its runtime `spec`. It is possible that the upper layer runtime
(i.e. containerd or CRI-O) may pass sandbox sizing annotations within the pause container's
`spec`. If these are provided, we will use this to appropriately size the VM. In particular,
we'll calculate the number of CPUs required for the workload and augment this by `default_vcpus`
configuration option, and use this for the virtual machine size.

In the case of a single container (i.e., not a pod), if the container specifies resource requirements,
the container's `spec` will provide the sizing information directly. If these are set, we will
calculate the number of CPUs required for the workload and augment this by `default_vcpus`
configuration option, and use this for the virtual machine size.

[1]: https://kubernetes.io/docs/tasks/configure-pod-container/assign-cpu-resource
[2]: https://kubernetes.io/docs/concepts/workloads/pods/pod/
[3]: ../../src/agent
[4]: ../../src/runtime
[5]: ../../src/runtime/README.md#configuration

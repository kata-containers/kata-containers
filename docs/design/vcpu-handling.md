- [Virtual machine vCPU sizing in Kata Containers](#virtual-machine-vcpu-sizing-in-kata-containers)
  * [Default number of virtual CPUs](#default-number-of-virtual-cpus)
  * [Virtual CPUs and Kubernetes pods](#virtual-cpus-and-kubernetes-pods)
  * [Container lifecycle](#container-lifecycle)
  * [Container without CPU constraint](#container-without-cpu-constraint)
  * [Container with CPU constraint](#container-with-cpu-constraint)
  * [Do not waste resources](#do-not-waste-resources)

# Virtual machine vCPU sizing in Kata Containers

## Default number of virtual CPUs

Before starting a container, the [runtime][6] reads the `default_vcpus` option
from the [configuration file][7] to determine the number of virtual CPUs
(vCPUs) needed to start the virtual machine. By default, `default_vcpus` is
equal to 1 for fast boot time and a small memory footprint per virtual machine.
Be aware that increasing this value negatively impacts the virtual machine's
boot time and memory footprint.
In general, we recommend that you do not edit this variable, unless you know
what are you doing. If your container needs more than one vCPU, use
[docker `--cpus`][1], [docker update][4], or [Kubernetes `cpu` limits][2] to
assign more resources.

*Docker*

```sh
$ docker run --name foo -ti --cpus 2 debian bash
$ docker update --cpus 4 foo
```


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
network, and a specification for how to run the containers [[specification][3]].
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
use [`docker --cpu-shares`][1] or [Kubernetes `cpu` requests][2].

*Docker*

```sh
$ docker run -ti --cpus-shares=512 debian bash
```

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
[agent][5]). In general, we recommend setting `default_vcpus` equal to 1 to
allow non-container processes to run on this vCPU and to specify a CPU
constraint for each container. If your container is already running and needs
more vCPUs, you can add more using [docker update][4].

## Container with CPU constraint

The runtime calculates the number of vCPUs required by a container with CPU
constraints using the following formula: `vCPUs = ceiling( quota / period )`, where
`quota` specifies the number of microseconds per CPU Period that the container is
guaranteed CPU access and `period` specifies the CPU CFS scheduler period of time
in microseconds. The result determines the number of vCPU to hot plug into the
virtual machine. Once the vCPUs have been added, the [agent][5] places the
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


*Container without CPU constraint*

```sh
$ docker run -ti debian bash -c "nproc; cat /sys/fs/cgroup/cpu,cpuacct/cpu.cfs_*"
1       # number of vCPUs
100000  # cfs period
-1      # cfs quota
```

*Container with CPU constraint*

```sh
docker run --cpus 4 -ti debian bash -c "nproc; cat /sys/fs/cgroup/cpu,cpuacct/cpu.cfs_*"
5       # number of vCPUs
100000  # cfs period
400000  # cfs quota
```


[1]: https://docs.docker.com/config/containers/resource_constraints/#cpu
[2]: https://kubernetes.io/docs/tasks/configure-pod-container/assign-cpu-resource
[3]: https://kubernetes.io/docs/concepts/workloads/pods/pod/
[4]: https://docs.docker.com/engine/reference/commandline/update/
[5]: ../../src/agent
[6]: ../../src/runtime
[7]: ../../src/runtime/README.md#configuration

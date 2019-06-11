* [CPU constraints in Kata Containers](#cpu-constraints-in-kata-containers)
    * [Default number of virtual CPUs](#default-number-of-virtual-cpus)
    * [Virtual CPUs and Kubernetes pods](#virtual-cpus-and-kubernetes-pods)
    * [Container lifecycle](#container-lifecycle)
    * [Container without CPU constraint](#container-without-cpu-constraint)
    * [Container with CPU constraint](#container-with-cpu-constraint)
    * [Do not waste resources](#do-not-waste-resources)
    * [CPU cgroups](#cpu-cgroups)
    * [cgroups in the guest](#cgroups-in-the-guest)
        * [CPU pinning](#cpu-pinning)
    * [cgroups in the host](#cgroups-in-the-host)


# CPU constraints in Kata Containers

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


## CPU cgroups

Kata Containers runs over two layers of cgroups, the first layer is in the guest where
only the workload is placed, the second layer is in the host that is more complex and
might contain more than one process and task (thread) depending of the number of
containers per POD and vCPUs per container. The following diagram represents a Nginx container
created with `docker` with the default number of vCPUs.


```
$ docker run -dt --runtime=kata-runtime nginx


       .-------.
       | Nginx |
    .--'-------'---.  .------------.
    | Guest Cgroup |  | Kata agent |
  .-'--------------'--'------------'.    .-----------.
  |  Thread: Hypervisor's vCPU 0    |    | Kata Shim |
 .'---------------------------------'.  .'-----------'.
 |             Tasks                 |  |  Processes  |
.'-----------------------------------'--'-------------'.
|                    Host Cgroup                       |
'------------------------------------------------------'
```

The next sections explain the difference between processes and tasks and why only hypervisor
vCPUs are constrained.

### cgroups in the guest

Only the workload process including all its threads are placed into CPU cgroups, this means
that `kata-agent` and `systemd` run without constraints in the guest.

#### CPU pinning

Kata Containers tries to apply and honor the cgroups but sometimes that is not possible.
An example of this occurs with CPU cgroups when the number of virtual CPUs (in the guest)
does not match the actual number of physical host CPUs.
In Kata Containers to have a good performance and small memory footprint, the resources are
hot added when they are needed, therefore the number of virtual resources is not the same
as the number of physical resources. The problem with this approach is that it's not possible
to pin a process on a specific resource that is not present in the guest. To deal with this
limitation and to not fail when the container is being created, Kata Containers does not apply
the constraint in the first layer (guest) if the resource does not exist in the guest, but it
is applied in the second layer (host) where the hypervisor is running. The constraint is applied
in both layers when the resource is available in the guest and host. The next sections provide
further details on what parts of the hypervisor are constrained.

### cgroups in the host

In Kata Containers the workloads run in a virtual machine that is managed and represented by a
hypervisor running in the host. Like other processes the hypervisor might use threads to realize
several tasks, for example IO and Network operations. One of the most important uses for the
threads is as vCPUs. The processes running in the guest see these vCPUs as physical CPUs, while
in the host those vCPU are just threads that are part of a process. This is the key to ensure
workloads consumes only the amount of CPU resources that were assigned to it without impacting
other operations. From user perspective the easier approach to implement it would be to take the
whole hypervisor including its threads and move them into the cgroup, unfortunately this will
impact negatively the performance, since vCPUs, IO and Network threads will be fighting for
resources. The following table shows a random read performance comparison between a Kata Container
with all its hypervisor threads in the cgroup and other with only its hypervisor vCPU threads
constrained, the difference is huge.


| Bandwidth     | All threads   | vCPU threads | Units |
|:-------------:|:-------------:|:------------:|:-----:|
| 4k            | 136.2         | 294.7        | MB/s  |
| 8k            | 166.6         | 579.4        | MB/s  |
| 16k           | 178.3         | 1093.3       | MB/s  |
| 32k           | 179.9         | 1931.5       | MB/s  |
| 64k           | 213.6         | 3994.2       | MB/s  |


To have the best performance in Kata Containers only the vCPU threads are constrained.


[1]: https://docs.docker.com/config/containers/resource_constraints/#cpu
[2]: https://kubernetes.io/docs/tasks/configure-pod-container/assign-cpu-resource
[3]: https://kubernetes.io/docs/concepts/workloads/pods/pod/
[4]: https://docs.docker.com/engine/reference/commandline/update/
[5]: https://github.com/kata-containers/agent
[6]: https://github.com/kata-containers/runtime
[7]: https://github.com/kata-containers/runtime#configuration

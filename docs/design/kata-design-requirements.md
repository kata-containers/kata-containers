## Design requirements

The Kata Containers runtime **MUST** fulfill all of the following requirements:

### OCI compatibility
The Kata Containers runtime **MUST** implement the [OCI runtime specification](https://github.com/opencontainers/runtime-spec) and support all
the OCI runtime operations.

### [`runc`](https://github.com/opencontainers/runc) CLI compatibility
In theory, being OCI compatible should be enough. In practice, the Kata Containers runtime
should comply with the latest *stable* `runc` CLI. In particular, it **MUST** implement the
following `runc` commands:

* `create`
* `delete`
* `exec`
* `kill`
* `list`
* `pause`
* `ps`
* `start`
* `state`
* `version`

The Kata Containers runtime **MUST** implement the following command line options:
* `--console-socket`
* `--pid-file`

### [CRI](http://blog.kubernetes.io/2016/12/container-runtime-interface-cri-in-kubernetes.html) and [Kubernetes](https://kubernetes.io) support
The Kata Containers project **MUST** provide two interfaces for CRI shims to manage hardware
virtualization based Kubernetes pods and containers:
-  An OCI and `runc` compatible command line interface, as described in the previous section.
This interface is used by implementations such as [`CRI-O`](http://cri-o.io) and [`containerd`](https://github.com/containerd/containerd), for example.
- A hardware virtualization runtime library API for CRI shims to consume and provide a more
CRI native implementation. The [`frakti`](https://github.com/kubernetes/frakti) CRI shim is an example of such a consumer.

### Multiple hardware architectures support
The Kata Containers runtime **MUST NOT** be architecture-specific. It should be able to support
multiple hardware architectures and provide a modular and flexible design for adding support
for additional ones.

### Multiple hypervisor support
The Kata Containers runtime **MUST NOT** be tied to any specific hardware virtualization technology,
hypervisor, or virtual machine monitor implementation.
It should support multiple hypervisors and provide a pluggable and flexible design to add support
for additional ones.

#### Nesting
The Kata Containers runtime **MUST** support nested virtualization environments.

### Networking

* The Kata Containers runtime **MUST** support CNI plugin.
* The Kata Containers runtime **MUST** support both legacy and IPv6 networks.

### I/O

#### Devices direct assignment
In order for containers to directly consume host hardware resources, the Kata Containers runtime
**MUST** provide containers with secure pass through for generic devices such as GPUs, SRIOV,
RDMA, QAT, by leveraging I/O virtualization technologies (IOMMU, interrupt remapping).

#### Acceleration
The Kata Containers runtime **MUST** support accelerated and user-space-based I/O operations
for networking (e.g. DPDK) as well as storage through `vhost-user` sockets.

#### Scalability
The Kata Containers runtime **MUST** support scalable I/O through the SRIOV technology.


### Virtualization overhead reduction
A compelling aspect of containers is their minimal overhead compared to bare metal applications.
A container runtime should keep the overhead to a minimum in order to provide the expected user
experience. 
The Kata Containers runtime implementation **SHOULD** be optimized for:

* Minimal workload boot and shutdown times
* Minimal workload memory footprint
* Maximal networking throughput
* Minimal networking latency

### Testing and debugging

#### Continuous Integration
Each Kata Containers runtime pull request **MUST** pass at least the following  set of container-related
tests:

* Unit tests: runtime unit tests coverage >75%
* Functional tests: the entire runtime CLI and APIs
* Integration tests: Docker and Kubernetes

#### Debugging

The Kata Containers runtime implementation **MUST** use structured logging in order to namespace
log messages to facilitate debugging.

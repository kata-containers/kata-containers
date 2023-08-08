# Kubernetes support

[Kubernetes](https://github.com/kubernetes/kubernetes/), or K8s, is a popular open source
container orchestration engine. In Kubernetes, a set of containers sharing resources
such as networking, storage, mount, PID, etc. is called a
[pod](https://kubernetes.io/docs/concepts/workloads/pods/).

A node can have multiple pods, but at a minimum, a node within a Kubernetes cluster
only needs to run a container runtime and a container agent (called a
[Kubelet](https://kubernetes.io/docs/concepts/overview/components/#kubelet)).

Kata Containers represents a Kubelet pod as a VM.

A Kubernetes cluster runs a control plane where a scheduler (typically
running on a dedicated control-plane node) calls into a compute Kubelet. This
Kubelet instance is responsible for managing the lifecycle of pods
within the nodes and eventually relies on a container runtime to
handle execution. The Kubelet architecture decouples lifecycle
management from container execution through a dedicated gRPC based
[Container Runtime Interface (CRI)](https://github.com/kubernetes/design-proposals-archive/blob/main/node/container-runtime-interface-v1.md).

In other words, a Kubelet is a CRI client and expects a CRI
implementation to handle the server side of the interface.
[CRI-O](https://github.com/kubernetes-incubator/cri-o) and
[containerd](https://github.com/containerd/containerd/) are CRI
implementations that rely on
[OCI](https://github.com/opencontainers/runtime-spec) compatible
runtimes for managing container instances.

Kata Containers is an officially supported CRI-O and containerd
runtime. Refer to the following guides on how to set up Kata
Containers with Kubernetes:

- [How to use Kata Containers and containerd](../../how-to/containerd-kata.md)
- [Run Kata Containers with Kubernetes](../../how-to/run-kata-with-k8s.md)

# Run Kata Containers with Kubernetes

* [Run Kata Containers with Kubernetes](#run-kata-containers-with-kubernetes)
  * [Prerequisites](#prerequisites)
  * [Install a CRI implementation](#install-a-cri-implementation)
     * [CRI-O](#cri-o)
        * [Kubernetes Runtime Class (CRI-O v1.12 )](#kubernetes-runtime-class-cri-o-v112)
        * [Untrusted annotation (until CRI-O v1.12)](#untrusted-annotation-until-cri-o-v112)
        * [Network namespace management](#network-namespace-management)
     * [containerd with CRI plugin](#containerd-with-cri-plugin)
  * [Install Kubernetes](#install-kubernetes)
     * [Configure for CRI-O](#configure-for-cri-o)
     * [Configure for containerd](#configure-for-containerd)
  * [Run a Kubernetes pod with Kata Containers](#run-a-kubernetes-pod-with-kata-containers)

## Prerequisites
This guide requires Kata Containers available on your system, install-able by following [this guide](../install/README.md).

## Install a CRI implementation

Kubernetes CRI (Container Runtime Interface) implementations allow using any
OCI-compatible runtime with Kubernetes, such as the Kata Containers runtime.

Kata Containers support both the [CRI-O](https://github.com/kubernetes-incubator/cri-o) and
[CRI-containerd](https://github.com/containerd/cri) CRI implementations.

After choosing one CRI implementation, you must make the appropriate configuration
to ensure it integrates with Kata Containers.

Kata Containers 1.5 introduced the `shimv2` for containerd 1.2.0, reducing the components
required to spawn pods and containers, and this is the preferred way to run Kata Containers with Kubernetes ([as documented here](../how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-containerd-to-use-kata-containers)).

An equivalent shim implementation for CRI-O is planned.

### CRI-O
For CRI-O installation instructions, refer to the [CRI-O Tutorial](https://github.com/kubernetes-incubator/cri-o/blob/master/tutorial.md) page.

The following sections show how to set up the CRI-O configuration file (default path: `/etc/crio/crio.conf`) for Kata.

Unless otherwise stated, all the following settings are specific to the `crio.runtime` table:
```toml
# The "crio.runtime" table contains settings pertaining to the OCI
# runtime used and options for how to set up and manage the OCI runtime.
[crio.runtime]
```
A comprehensive documentation of the configuration file can be found [here](https://github.com/cri-o/cri-o/blob/master/docs/crio.conf.5.md).

> **Note**: After any change to this file, the CRI-O daemon have to be restarted with:
>````
>$ sudo systemctl restart crio
>````

#### Kubernetes Runtime Class (CRI-O v1.12+)
The [Kubernetes Runtime Class](https://kubernetes.io/docs/concepts/containers/runtime-class/)
is the preferred way of specifying the container runtime configuration to run a Pod's containers.
To use this feature, Kata must added as a runtime handler with:

```toml
[crio.runtime.runtimes.kata-runtime]
  runtime_path = "/usr/bin/kata-runtime"
  runtime_type = "oci"
```

You can also add multiple entries to specify alternatives hypervisors, e.g.:
```toml
[crio.runtime.runtimes.kata-qemu]
  runtime_path = "/usr/bin/kata-runtime"
  runtime_type = "oci"

[crio.runtime.runtimes.kata-fc]
  runtime_path = "/usr/bin/kata-runtime"
  runtime_type = "oci"
```

#### Untrusted annotation (until CRI-O v1.12)
The untrusted annotation is used to specify a runtime for __untrusted__ workloads, i.e.
a runtime to be used when the workload cannot be trusted and a higher level of security
is required. An additional flag can be used to let CRI-O know if a workload
should be considered _trusted_ or _untrusted_ by default.
For further details, see the documentation
[here](../design/architecture.md#mixing-vm-based-and-namespace-based-runtimes).

```toml
# runtime is the OCI compatible runtime used for trusted container workloads.
# This is a mandatory setting as this runtime will be the default one
# and will also be used for untrusted container workloads if
# runtime_untrusted_workload is not set.
runtime = "/usr/bin/runc"

# runtime_untrusted_workload is the OCI compatible runtime used for untrusted
# container workloads. This is an optional setting, except if
# default_container_trust is set to "untrusted".
runtime_untrusted_workload = "/usr/bin/kata-runtime"

# default_workload_trust is the default level of trust crio puts in container
# workloads. It can either be "trusted" or "untrusted", and the default
# is "trusted".
# Containers can be run through different container runtimes, depending on
# the trust hints we receive from kubelet:
# - If kubelet tags a container workload as untrusted, crio will try first to
# run it through the untrusted container workload runtime. If it is not set,
# crio will use the trusted runtime.
# - If kubelet does not provide any information about the container workload trust
# level, the selected runtime will depend on the default_container_trust setting.
# If it is set to "untrusted", then all containers except for the host privileged
# ones, will be run by the runtime_untrusted_workload runtime. Host privileged
# containers are by definition trusted and will always use the trusted container
# runtime. If default_container_trust is set to "trusted", crio will use the trusted
# container runtime for all containers.
default_workload_trust = "untrusted"
```

#### Network namespace management
To enable networking for the workloads run by Kata, CRI-O needs to be configured to
manage network namespaces, by setting the following key to `true`.

In CRI-O v1.16:
```toml
manage_network_ns_lifecycle = true
```
In CRI-O v1.17+:
```toml
manage_ns_lifecycle = true
```


### containerd with CRI plugin

If you select containerd with `cri` plugin, follow the "Getting Started for Developers"
instructions [here](https://github.com/containerd/cri#getting-started-for-developers)
to properly install it.

To customize containerd to select Kata Containers runtime, follow our
"Configure containerd to use Kata Containers" internal documentation
[here](../how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-containerd-to-use-kata-containers).

## Install Kubernetes

Depending on what your needs are and what you expect to do with Kubernetes,
please refer to the following
[documentation](https://kubernetes.io/docs/setup/) to install it correctly.

Kubernetes talks with CRI implementations through a `container-runtime-endpoint`,
also called CRI socket. This socket path is different depending on which CRI
implementation you chose, and the Kubelet service has to be updated accordingly.

### Configure for CRI-O

`/etc/systemd/system/kubelet.service.d/0-crio.conf`
```
[Service]
Environment="KUBELET_EXTRA_ARGS=--container-runtime=remote --runtime-request-timeout=15m --container-runtime-endpoint=unix:///var/run/crio/crio.sock"
```

### Configure for containerd

`/etc/systemd/system/kubelet.service.d/0-cri-containerd.conf`
```
[Service]
Environment="KUBELET_EXTRA_ARGS=--container-runtime=remote --runtime-request-timeout=15m --container-runtime-endpoint=unix:///run/containerd/containerd.sock"
```
For more information about containerd see the "Configure Kubelet to use containerd"
documentation [here](../how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-kubelet-to-use-containerd).

## Run a Kubernetes pod with Kata Containers

After you update your Kubelet service based on the CRI implementation you
are using, reload and restart Kubelet. Then, start your cluster:
```bash
$ sudo systemctl daemon-reload
$ sudo systemctl restart kubelet

# If using CRI-O
$ sudo kubeadm init --ignore-preflight-errors=all --cri-socket /var/run/crio/crio.sock --pod-network-cidr=10.244.0.0/16

# If using CRI-containerd
$ sudo kubeadm init --ignore-preflight-errors=all --cri-socket /run/containerd/containerd.sock --pod-network-cidr=10.244.0.0/16

$ export KUBECONFIG=/etc/kubernetes/admin.conf
```

You can force Kubelet to use Kata Containers by adding some `untrusted`
annotation to your pod configuration. In our case, this ensures Kata
Containers is the selected runtime to run the described workload.

`nginx-untrusted.yaml`
```yaml
apiVersion: v1
kind: Pod
metadata:
  name: nginx-untrusted
  annotations:
    io.kubernetes.cri.untrusted-workload: "true"
spec:
  containers:
    - name: nginx
      image: nginx
```

Next, you run your pod:
```
$ sudo -E kubectl apply -f nginx-untrusted.yaml
```


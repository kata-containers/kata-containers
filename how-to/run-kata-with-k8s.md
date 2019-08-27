# Run Kata Containers with Kubernetes

* [Pre-requisites](#pre-requisites)
* [Run Kata Containers with Kubernetes](#run-kata-containers-with-kubernetes)
  * [Install a CRI implementation](#install-a-cri-implementation)
     * [CRI-O](#cri-o)
     * [containerd with CRI plugin](#containerd-with-cri-plugin)
  * [Install Kubernetes](#install-kubernetes)
     * [Configure for CRI-O](#configure-for-cri-o)
     * [Configure for containerd](#configure-for-containerd)
  * [Run a Kubernetes pod with Kata Containers](#run-a-kubernetes-pod-with-kata-containers)

## Pre-requisites
This guide requires Kata Containers available on your system, and it can be installed
following [this guide](https://github.com/kata-containers/documentation/blob/master/install/README.md).


## Install a CRI implementation

Kata Containers runtime is an OCI compatible runtime and cannot directly
interact with the CRI API level. For this reason we rely on a CRI
implementation to translate CRI into OCI. There are two supported ways
called [CRI-O](https://github.com/kubernetes-incubator/cri-o) and
[CRI-containerd](https://github.com/containerd/cri). It is up to you to
choose the one that you want, but you have to pick one. After choosing
either CRI-O or CRI-containerd, you must make the appropriate changes
to ensure it relies on the Kata Containers runtime.

As of Kata Containers 1.5, using `shimv2` with containerd 1.2.0 or above is the preferred
way to run Kata Containers with Kubernetes ([see the howto](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-containerd-to-use-kata-containers)).
The CRI-O will catch up soon.

### CRI-O

If you select CRI-O, follow the "CRI-O Tutorial" instructions
[here](https://github.com/kubernetes-incubator/cri-o/blob/master/tutorial.md)
to properly install it.

Once you have installed CRI-O, you need to modify the CRI-O configuration
with information about different container runtimes. By default, we choose
`runc`, but in this case we also specify Kata Containers runtime to run
__untrusted__ workloads. In other words, this defines an alternative runtime
to be used when the workload cannot be trusted and a higher level of security
is required. An additional flag can be used to let CRI-O know if a workload
should be considered _trusted_ or _untrusted_ by default.
For further details, see the documentation
[here](https://github.com/kata-containers/documentation/blob/master/design/architecture.md#mixing-vm-based-and-namespace-based-runtimes).

Additionally, we need CRI-O to perform the network namespace management.
Otherwise, when the VM starts the network will not be available.

The following is an example of how to modify the `/etc/crio/crio.conf` file
in order to apply the previous explanations, and therefore get Kata Containers
runtime to invoke by CRI-O.

```toml
# The "crio.runtime" table contains settings pertaining to the OCI
# runtime used and options for how to set up and manage the OCI runtime.
[crio.runtime]
manage_network_ns_lifecycle = true

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

Restart CRI-O to take changes into account
```
$ sudo systemctl restart crio
```

### containerd with CRI plugin

If you select containerd with `cri` plugin, follow the "Getting Started for Developers"
instructions [here](https://github.com/containerd/cri#getting-started-for-developers)
to properly install it.

To customize containerd to select Kata Containers runtime, follow our
"Configure containerd to use Kata Containers" internal documentation
[here](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-containerd-to-use-kata-containers).

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
documentation [here](https://github.com/kata-containers/documentation/blob/master/how-to/how-to-use-k8s-with-cri-containerd-and-kata.md#configure-kubelet-to-use-containerd).

## Run a Kubernetes pod with Kata Containers

After you update your Kubelet service based on the CRI implementation you
are using, reload and restart Kubelet. Then, start your cluster:
```bash
$ sudo systemctl daemon-reload
$ sudo systemctl restart kubelet

# If using CRI-O
$ sudo kubeadm init --skip-preflight-checks --cri-socket /var/run/crio/crio.sock --pod-network-cidr=10.244.0.0/16

# If using CRI-containerd
$ sudo kubeadm init --skip-preflight-checks --cri-socket /run/containerd/containerd.sock --pod-network-cidr=10.244.0.0/16

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


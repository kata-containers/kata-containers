# Run Kata Containers with Kubernetes

## Prerequisites
This guide requires Kata Containers available on your system, install-able by following [this guide](../install/README.md).

## Install a CRI implementation

Kubernetes CRI (Container Runtime Interface) implementations allow using any
OCI-compatible runtime with Kubernetes, such as the Kata Containers runtime.

Kata Containers support both the [CRI-O](https://github.com/kubernetes-incubator/cri-o) and
[containerd](https://github.com/containerd/containerd) CRI implementations.

After choosing one CRI implementation, you must make the appropriate configuration
to ensure it integrates with Kata Containers.

Kata Containers 1.5 introduced the `shimv2` for containerd 1.2.0, reducing the components
required to spawn pods and containers, and this is the preferred way to run Kata Containers with Kubernetes ([as documented here](../how-to/how-to-use-k8s-with-containerd-and-kata.md#configure-containerd-to-use-kata-containers)).

An equivalent shim implementation for CRI-O is planned.

### CRI-O
For CRI-O installation instructions, refer to the [CRI-O Tutorial](https://github.com/cri-o/cri-o/blob/main/tutorial.md) page.

The following sections show how to set up the CRI-O snippet configuration file (default path: `/etc/crio/crio.conf`) for Kata.

Unless otherwise stated, all the following settings are specific to the `crio.runtime` table:
```toml
# The "crio.runtime" table contains settings pertaining to the OCI
# runtime used and options for how to set up and manage the OCI runtime.
[crio.runtime]
```
A comprehensive documentation of the configuration file can be found [here](https://github.com/cri-o/cri-o/blob/main/docs/crio.conf.5.md).

> **Note**: After any change to this file, the CRI-O daemon have to be restarted with:
>````
>$ sudo systemctl restart crio
>````

#### Kubernetes Runtime Class (CRI-O v1.12+)
The [Kubernetes Runtime Class](https://kubernetes.io/docs/concepts/containers/runtime-class/)
is the preferred way of specifying the container runtime configuration to run a Pod's containers.
To use this feature, Kata must added as a runtime handler. This can be done by
dropping a `50-kata` snippet file into `/etc/crio/crio.conf.d`, with the
content shown below:

```toml
[crio.runtime.runtimes.kata]
	runtime_path = "/usr/bin/containerd-shim-kata-v2"
	runtime_type = "vm"
	runtime_root = "/run/vc"
	privileged_without_host_devices = true
```


### containerd

To customize containerd to select Kata Containers runtime, follow our
"Configure containerd to use Kata Containers" internal documentation
[here](../how-to/how-to-use-k8s-with-containerd-and-kata.md#configure-containerd-to-use-kata-containers).

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
documentation [here](../how-to/how-to-use-k8s-with-containerd-and-kata.md#configure-kubelet-to-use-containerd).

## Run a Kubernetes pod with Kata Containers

After you update your Kubelet service based on the CRI implementation you
are using, reload and restart Kubelet. Then, start your cluster:
```bash
$ sudo systemctl daemon-reload
$ sudo systemctl restart kubelet

# If using CRI-O
$ sudo kubeadm init --ignore-preflight-errors=all --cri-socket /var/run/crio/crio.sock --pod-network-cidr=10.244.0.0/16

# If using containerd
$ cat <<EOF | tee kubeadm-config.yaml
apiVersion: kubeadm.k8s.io/v1beta3
kind: InitConfiguration
nodeRegistration:
  criSocket: "/run/containerd/containerd.sock"
---
kind: KubeletConfiguration
apiVersion: kubelet.config.k8s.io/v1beta1
cgroupDriver: cgroupfs
podCIDR: "10.244.0.0/16"
EOF
$ sudo kubeadm init --ignore-preflight-errors=all --config kubeadm-config.yaml

$ export KUBECONFIG=/etc/kubernetes/admin.conf
```

### Allow pods to run in the control-plane node

By default, the cluster will not schedule pods in the control-plane node. To enable control-plane node scheduling:
```bash
$ sudo -E kubectl taint nodes --all node-role.kubernetes.io/control-plane-
```

### Create runtime class for Kata Containers

Users can use [`RuntimeClass`](https://kubernetes.io/docs/concepts/containers/runtime-class/#runtime-class) to specify a different runtime for Pods.

```bash
$ cat > runtime.yaml <<EOF
apiVersion: node.k8s.io/v1
kind: RuntimeClass
metadata:
  name: kata
handler: kata
EOF

$ sudo -E kubectl apply -f runtime.yaml
```

### Run pod in Kata Containers

If a pod has the `runtimeClassName` set to `kata`, the CRI plugin runs the pod with the
[Kata Containers runtime](../../src/runtime/README.md).

- Create an pod configuration that using Kata Containers runtime

  ```bash
  $ cat << EOF | tee nginx-kata.yaml
  apiVersion: v1
  kind: Pod
  metadata:
    name: nginx-kata
  spec:
    runtimeClassName: kata
    containers:
    - name: nginx
      image: nginx

  EOF
  ```

- Create the pod
  ```bash
  $ sudo -E kubectl apply -f nginx-kata.yaml
  ```

- Check pod is running

  ```bash
  $ sudo -E kubectl get pods
  ```

- Check hypervisor is running
  ```bash
  $ ps aux | grep qemu
  ```

### Delete created pod

```bash
$ sudo -E kubectl delete -f nginx-kata.yaml
```

# How to use Kata Containers and containerd with Kubernetes

This document describes how to set up a single-machine Kubernetes (k8s) cluster.

The Kubernetes cluster will use the
[containerd](https://github.com/containerd/containerd/) and
[Kata Containers](https://katacontainers.io) to launch workloads.

## Requirements

- Kubernetes, Kubelet, `kubeadm`
- containerd
- Kata Containers

> **Note:** For information about the supported versions of these components,
> see the  Kata Containers
> [`versions.yaml`](../../versions.yaml)
> file.

## Install and configure containerd

First, follow the [How to use Kata Containers and Containerd](containerd-kata.md) to install and configure containerd. 
Then, make sure the containerd works with the [examples in it](containerd-kata.md#run).

## Install and configure Kubernetes

### Install Kubernetes

- Follow the instructions for
  [`kubeadm` installation](https://kubernetes.io/docs/setup/independent/install-kubeadm/).

- Check `kubeadm` is now available

  ```bash
  $ command -v kubeadm
  ```

### Configure Kubelet to use containerd

In order to allow Kubelet to use containerd (using the CRI interface), configure the service to point to the `containerd` socket.

- Configure Kubernetes to use `containerd`

  ```bash
  $ sudo mkdir -p  /etc/systemd/system/kubelet.service.d/
  $ cat << EOF | sudo tee  /etc/systemd/system/kubelet.service.d/0-containerd.conf
  [Service]                                                 
  Environment="KUBELET_EXTRA_ARGS=--container-runtime=remote --runtime-request-timeout=15m --container-runtime-endpoint=unix:///run/containerd/containerd.sock"
  EOF
  ```

- Inform systemd about the new configuration

  ```bash
  $ sudo systemctl daemon-reload
  ```

### Configure HTTP proxy - OPTIONAL

If you are behind a proxy, use the following script to configure your proxy for docker, Kubelet, and containerd:

```bash
$ services="
kubelet
containerd
docker
"

$ for service in ${services}; do

    service_dir="/etc/systemd/system/${service}.service.d/"
    sudo mkdir -p ${service_dir}

    cat << EOF | sudo tee "${service_dir}/proxy.conf"
[Service]
Environment="HTTP_PROXY=${http_proxy}"
Environment="HTTPS_PROXY=${https_proxy}"
Environment="NO_PROXY=${no_proxy}"
EOF
done

$ sudo systemctl daemon-reload
```

## Start Kubernetes

- Make sure `containerd` is up and running

  ```bash
  $ sudo systemctl restart containerd
  $ sudo systemctl status containerd
  ```

- Prevent conflicts between `docker` iptables (packet filtering) rules and k8s pod communication

  If Docker is installed on the node, it is necessary to modify the rule
  below. See https://github.com/kubernetes/kubernetes/issues/40182 for further
  details.

  ```bash
  $ sudo iptables -P FORWARD ACCEPT
  ```

- Start cluster using `kubeadm`

  ```bash
  $ sudo kubeadm init --cri-socket /run/containerd/containerd.sock --pod-network-cidr=10.244.0.0/16
  $ export KUBECONFIG=/etc/kubernetes/admin.conf
  $ sudo -E kubectl get nodes
  $ sudo -E kubectl get pods
  ```

## Configure Pod Network

A pod network plugin is needed to allow pods to communicate with each other.
You can find more about CNI plugins from the [Creating a cluster with `kubeadm`](https://kubernetes.io/docs/setup/independent/create-cluster-kubeadm/#instructions) guide.

By default the CNI plugin binaries is installed under `/opt/cni/bin` (in package `kubernetes-cni`), you only need to create a configuration file for CNI plugin.

  ```bash
  $ sudo -E mkdir -p /etc/cni/net.d

  $ sudo -E cat > /etc/cni/net.d/10-mynet.conf <<EOF
  {
    "cniVersion": "0.2.0",
    "name": "mynet",
    "type": "bridge",
    "bridge": "cni0",
    "isGateway": true,
    "ipMasq": true,
    "ipam": {
      "type": "host-local",
      "subnet": "172.19.0.0/24",
      "routes": [
        { "dst": "0.0.0.0/0" }
      ]
    }
  }
  EOF
  ```

## Allow pods to run in the control-plane node

By default, the cluster will not schedule pods in the control-plane node. To enable control-plane node scheduling:

```bash
$ sudo -E kubectl taint nodes --all node-role.kubernetes.io/control-plane-
```

## Create runtime class for Kata Containers

By default, all pods are created with the default runtime configured in containerd.
From Kubernetes v1.12, users can use [`RuntimeClass`](https://kubernetes.io/docs/concepts/containers/runtime-class/#runtime-class) to specify a different runtime for Pods.

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

## Run pod in Kata Containers

If a pod has the `runtimeClassName` set to `kata`, the CRI runs the pod with the
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

## Delete created pod

```bash
$ sudo -E kubectl delete -f nginx-kata.yaml
```

# How to use Kata Containers and CRI (containerd plugin) with Kubernetes

* [Requirements](#requirements)
* [Install and configure containerd](#install-and-configure-containerd)
* [Install and configure Kubernetes](#install-and-configure-kubernetes)
    * [Install Kubernetes](#install-kubernetes)
    * [Configure Kubelet to use containerd](#configure-kubelet-to-use-containerd)
    * [Configure HTTP proxy - OPTIONAL](#configure-http-proxy---optional)
* [Start Kubernetes](#start-kubernetes)
* [Install a Pod Network](#install-a-pod-network)
* [Allow pods to run in the master node](#allow-pods-to-run-in-the-master-node)
* [Create an untrusted pod using Kata Containers](#create-an-untrusted-pod-using-kata-containers)
* [Delete created pod](#delete-created-pod)

This document describes how to set up a single-machine Kubernetes (k8s) cluster.

The Kubernetes cluster will use the
[CRI containerd plugin](https://github.com/containerd/cri) and
[Kata Containers](https://katacontainers.io) to launch untrusted workloads.

For Kata Containers 1.5.0-rc2 and above, we will use `containerd-shim-kata-v2` (short as `shimv2` in this documentation)
to launch Kata Containers. For the previous version of Kata Containers, the Pods are launched with `kata-runtime`.

## Requirements

- Kubernetes, Kubelet, `kubeadm`
- containerd with `cri` plug-in
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

    cat << EOT | sudo tee "${service_dir}/proxy.conf"
[Service]
Environment="HTTP_PROXY=${http_proxy}"
Environment="HTTPS_PROXY=${https_proxy}"
Environment="NO_PROXY=${no_proxy}"
EOT
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

## Install a Pod Network

A pod network plugin is needed to allow pods to communicate with each other.

- Install the `flannel` plugin by following the
  [Using `kubeadm` to Create a Cluster](https://kubernetes.io/docs/setup/independent/create-cluster-kubeadm/#instructions)
  guide, starting from the **Installing a pod network** section.

- Create a pod network using flannel

  > **Note:** There is no known way to determine programmatically the best version (commit) to use.
  > See https://github.com/coreos/flannel/issues/995.

  ```bash
  $ sudo -E kubectl apply -f https://raw.githubusercontent.com/coreos/flannel/master/Documentation/kube-flannel.yml
  ```

- Wait for the pod network to become available

  ```bash
  # number of seconds to wait for pod network to become available
  $ timeout_dns=420

  $ while [ "$timeout_dns" -gt 0 ]; do
      if sudo -E kubectl get pods --all-namespaces | grep dns | grep Running; then
          break
      fi

      sleep 1s
      ((timeout_dns--))
   done
  ```

- Check the pod network is running

  ```bash
  $ sudo -E kubectl get pods --all-namespaces | grep dns | grep Running && echo "OK" || ( echo "FAIL" && false )
  ```

## Allow pods to run in the master node

By default, the cluster will not schedule pods in the master node. To enable master node scheduling:

```bash
$ sudo -E kubectl taint nodes --all node-role.kubernetes.io/master-
```

## Create an untrusted pod using Kata Containers

By default, all pods are created with the default runtime configured in CRI containerd plugin.

If a pod has the `io.kubernetes.cri.untrusted-workload` annotation set to `"true"`, the CRI plugin runs the pod with the
[Kata Containers runtime](../../src/runtime/README.md).

- Create an untrusted pod configuration

  ```bash
  $ cat << EOT | tee nginx-untrusted.yaml
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
      
  EOT
  ```

- Create an untrusted pod
  ```bash
  $ sudo -E kubectl apply -f nginx-untrusted.yaml
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
$ sudo -E kubectl delete -f nginx-untrusted.yaml
```

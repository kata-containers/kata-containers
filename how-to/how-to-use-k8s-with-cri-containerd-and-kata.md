# How to use Kata Containers and CRI (containerd plugin) with Kubernetes

* [Requirements](#requirements)
* [Install containerd with CRI plugin enabled](#install-containerd-with-cri-plugin-enabled)
* [Install Kata Containers](#install-kata-containers)
* [Install Kubernetes](#install-kubernetes)
* [Configure containerd to use Kata Containers](#configure-containerd-to-use-kata-containers)
    * [Define the Kata runtime as the untrusted workload runtime](#define-the-kata-runtime-as-the-untrusted-workload-runtime)
* [Configure Kubelet to use containerd](#configure-kubelet-to-use-containerd)
* [Configure proxy - OPTIONAL](#configure-proxy---optional)
* [Start Kubernetes](#start-kubernetes)
* [Install a Pod Network](#install-a-pod-network)
* [Allow pods to run in the master node](#allow-pods-to-run-in-the-master-node)
* [Create an unstrusted pod using Kata Containers](#create-an-unstrusted-pod-using-kata-containers)
* [Delete created pod](#delete-created-pod)

This document describes how to set up a single-machine Kubernetes (k8s) cluster.

The Kubernetes cluster will use the
[CRI containerd plugin](https://github.com/containerd/cri) and
[Kata Containers](https://katacontainers.io) to launch untrusted workloads.

## Requirements

- Kubernetes, kubelet, kubeadm
- cri-containerd
- Kata Containers

> **Note:** For information about the supported versions of these components,
> see the  Kata Containers
> [versions.yaml](https://github.com/kata-containers/runtime/blob/master/versions.yaml)
> file.

## Install containerd with CRI plugin enabled

- Follow the instructions from the
  [CRI installation guide](http://github.com/containerd/cri/blob/master/docs/installation.md).

- Check if `containerd` is now available
  ```bash
  $ command -v containerd
  ```

## Install Kata Containers

Follow the instructions to
[install Kata Containers](https://github.com/kata-containers/documentation/blob/master/install/README.md).

## Install Kubernetes

- Follow the instructions for
  [kubeadm installation](https://kubernetes.io/docs/setup/independent/install-kubeadm/).

- Check `kubeadm` is now available

  ```bash
  $ command -v kubeadm
  ```

## Configure containerd to use Kata Containers

The CRI `containerd` plugin supports configuration for two runtime types.

- **Default runtime:**

  A runtime that is used by default to run workloads.

- **Untrusted workload runtime:**

  A runtime that will be used to run untrusted workloads. This is appropriate
  for workloads that require a higher degree of security isolation.

#### Define the Kata runtime as the untrusted workload runtime

Configure `containerd` to use the Kata runtime to run untrusted workloads by
setting the `plugins.cri.containerd.untrusted_workload_runtime`
[config option](https://github.com/containerd/cri/blob/v1.0.0-rc.0/docs/config.md):

```bash
$ sudo mkdir -p /etc/containerd/
$ cat << EOT | sudo tee /etc/containerd/config.toml
[plugins]
    [plugins.cri.containerd]
      [plugins.cri.containerd.untrusted_workload_runtime]
        runtime_type = "io.containerd.runtime.v1.linux"
        runtime_engine = "/usr/bin/kata-runtime"
EOT
```

> **Note:** Unless configured otherwise, the default runtime is set to `runc`.

## Configure Kubelet to use containerd

In order to allow kubelet to use containerd (using the CRI interface), configure the service to point to the `containerd` socket.

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

## Configure proxy - OPTIONAL

If you are behind a proxy, use the following script to configure your proxy for docker, kubelet, and containerd:

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
  $ sudo kubeadm init --skip-preflight-checks --cri-socket /run/containerd/containerd.sock --pod-network-cidr=10.244.0.0/16
  $ export KUBECONFIG=/etc/kubernetes/admin.conf
  $ sudo -E kubectl get nodes
  $ sudo -E kubectl get pods
  ```

## Install a Pod Network

A pod network plugin is needed to allow pods to communicate with each other.

- Install the `flannel` plugin by following the
  [Using kubeadm to Create a Cluster](https://kubernetes.io/docs/setup/independent/create-cluster-kubeadm/#instructions)
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

## Create an unstrusted pod using Kata Containers

By default, all pods are created with the default runtime configured in CRI containerd plugin.

If a pod has the `io.kubernetes.cri.untrusted-workload` annotation set to `"true"`, the CRI plugin runs the pod with the
[Kata Containers runtime](https://github.com/kata-containers/runtime/blob/master/README.md).

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

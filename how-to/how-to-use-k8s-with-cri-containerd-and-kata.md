# How to use Kata Containers and CRI (containerd plugin) with Kubernetes

This document describes how to set up a single-machine Kubernetes cluster. 
The Kubernetes cluster will use the CRI containerd plugin and Kata Containers to launch untrusted workloads.

## Requirements 
- Kubernetes, kubelet, kubeadm
- cri-containerd
- Kata Containers

For information about the supported version of these components see 
Kata Containers [versions.yaml](https://github.com/kata-containers/runtime/blob/master/versions.yaml) file.

## Install containerd(with CRI plugin enabled)

Follow the instructions from [CRI installation guide](http://github.com/containerd/cri/blob/master/docs/installation.md)

<!---
```bash
# Check if containerd is installed
$ command -v containerd
```
--->

## Install Kata Containers 

Follow the instructions to [install Kata](https://github.com/kata-containers/documentation/blob/master/install/README.md).

<!---
```bash
# Check if kata-runtime is installed
$ command -v kata-runtime
# Check kata is well configured
$ kata-runtime kata-env
```
--->

## Install Kubernetes
Install Kubernetes in your host.  See kubeadm [installation](https://kubernetes.io/docs/tasks/tools/install-kubeadm/)
<!---
```bash
# Check if kubadm is installed
$ command -v kubeadm
```
--->

### Configure containerd to use Kata Containers

The CRI containerd plugin support configuration for two runtime types.

- Default runtime: A runtime that is used by default to run workloads.
- Untrusted workload runtime: A runtime that will be used run untrusted workloads.

#### Define the Kata runtime as `untrusted_workload_runtime` 

Configure the Kata runtime for untrusted workload with the [config option](https://github.com/containerd/cri/blob/v1.0.0-rc.0/docs/config.md)
`plugins.cri.containerd.untrusted_workload_runtime`.

Unless configured otherwise, the default runtime is set to `runc`.
```bash
# Configure containerd to use Kata as untrusted_workload_runtime
$ sudo mkdir -p /etc/containerd/
$ cat << EOT | sudo tee /etc/containerd/config.toml
[plugins]
    [plugins.cri.containerd]
      [plugins.cri.containerd.untrusted_workload_runtime]
        runtime_type = "io.containerd.runtime.v1.linux"
        runtime_engine = "/usr/bin/kata-runtime"
EOT
```

### Configure Kubelet to use containerd

In order to allow kubelet use containerd (using CRI interface) configure the service to
point to containerd socket.

```bash
# Configure k8s to use containerd
$ sudo mkdir -p  /etc/systemd/system/kubelet.service.d/
$ cat << EOF | sudo tee  /etc/systemd/system/kubelet.service.d/0-containerd.conf
[Service]                                                 
Environment="KUBELET_EXTRA_ARGS=--container-runtime=remote --runtime-request-timeout=15m --container-runtime-endpoint=unix:///run/containerd/containerd.sock"
EOF
$ sudo systemctl daemon-reload
```

### Optional: Configure proxy

If you are behind a proxy this script will configure your proxy for docker
kubelet and containerd.

```bash
# Set proxys
$ services=(
'kubelet'
'containerd'
'docker'
)

$ for s in "${services[@]}"; do 

	service_dir="/etc/systemd/system/${s}.service.d/"
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

###  Start Kubernetes with kubeadm 

```bash
# Mark sure containerd is up and running
$ sudo systemctl restart containerd
$ sudo systemctl status containerd

# Prevent docker iptables rules conflict with k8s pod communication
$ sudo iptables -P FORWARD ACCEPT

# Start cluster using kubeadm
$ sudo kubeadm init --skip-preflight-checks \
--cri-socket /run/containerd/containerd.sock --pod-network-cidr=10.244.0.0/16

$ export KUBECONFIG=/etc/kubernetes/admin.conf

$ sudo -E kubectl get nodes
$ sudo -E kubectl get pods
```

### Install a pod network
Install a pod network plugin is needed to allow pods communicate with each other.

Install flannel plugging, by following the instructions in the section *Installing a pod network*
from [Using kubeadm to Create a Cluster ](https://kubernetes.io/docs/setup/independent/create-cluster-kubeadm/)
guide.

<!---
```bash
# Install a pod network using flannel
# There is not a programmatic way to know last what flannel commit use
# See https://github.com/coreos/flannel/issues/995
$ sudo -E kubectl apply -f https://raw.githubusercontent.com/coreos/flannel/master/Documentation/kube-flannel.yml
```
--->


```bash
# wait for pod network
$ timeout_dns=0
$ until [ "$timeout_dns" -eq "420" ]; do
	if sudo -E kubectl get pods --all-namespaces | grep dns | grep Running; then
		break
	fi
	sleep 1s
	((timeout_dns+=1))
 done

# check pod network is running
$ sudo -E kubectl get pods --all-namespaces | grep dns | grep Running && echo "OK" || ( echo "FAIL" && false )
```

### Allow run pods in master node

By default, the cluster will not schedule pods in the master node to allow that run:

```bash
# allow master node run pods
$ sudo -E kubectl taint nodes --all node-role.kubernetes.io/master-
```


### Create a unstrusted pod using Kata Containers

By default, all pods are created with the default runtime configured in CRI containerd plugin.
If a pod has the `io.kubernetes.cri.untrusted-workload annotation` set as
`"true"`, the CRI plugin will run the pod with the Kata Containers runtime.

```bash
# Create untrusted pod configuration
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

# Create untrusted pod
$ sudo -E kubectl apply -f nginx-untrusted.yaml

# Check pod is running
$ sudo -E kubectl get pods

# Check qemu is running
$ ps aux | grep qemu
```
### Delete created pod

```bash
# Delete pod
$ sudo -E kubectl delete -f  nginx-untrusted.yaml
```

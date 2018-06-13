# kata-deploy


- [kata-deploy](#kata-deploy)
  * [Quick start](#quick-start-)
    + [Install Kata on a running Kubernetes cluster](#install-kata-on-a-running-kubernetes-cluster)
    + [Run a sample workload](#run-a-sample-workload-)
    + [Remove Kata from the Kubernetes cluster](#remove-kata-from-the-kubernetes-cluster-)
  * [kata-deploy details](#kata-deploy-details)
    + [Dockerfile](#dockerfile)
    + [Daemonsets and RBAC](#daemonsets-and-rbac-)
      - [runtime-labeler](#runtime-labeler-)
      - [CRI-O and containerd kata installer](#cri-o-and-containerd-kata-installer-)
    + [Kata cleanup](#kata-cleanup-)


[kata-deploy](kata-deploy) provides a Dockerfile which contains all of the binaries
and artifacts required to run Kata Containers, as well as reference daemonsets which can be utilized to install Kata Containers on a running Kubernetes cluster.

Note, installation through daemonsets only succesfully installs `kata-containers.io/kata-runtime` on
a node if it uses either containerd or CRI-O CRI-shims.

## Quick start:

### Install Kata on a running Kubernetes cluster

```
kubectl apply -f kata-rbac.yaml
kubectl apply -f kata-deploy.yaml
```

### Run a sample workload

Untrusted workloads can node-select based on ```kata-containers.io/kata-runtime=true```, and are
run through ```kata-containers.io/kata-runtime``` if they are marked with the appropriate CRIO or containerd
annotation:
```
CRIO:           io.kubernetes.cri-o.TrustedSandbox: "false"
containerd:     io.kubernetes.cri.untrusted-workload: "true"
```

The following is a sample workload for running untrusted on a kata-enabled node:
```
apiVersion: v1
kind: Pod
metadata:
  name: nginx
   annotations:
    io.kubernetes.cri-o.TrustedSandbox: "false"
    io.kubernetes.cri.untrusted-workload: "true"
  labels:
    env: test
spec:
  containers:
  - name: nginx
    image: nginx
    imagePullPolicy: IfNotPresent
  nodeSelector:
    kata-containers.io/kata-runtime: "true"
```    

To run:
```
kubectl apply -f examples/nginx-untrusted.yaml
```

Now, you should see the pod start. You can verify that the pod is making use of
```kata-containers.io/kata-runtime``` by comparing the container ID observed with the following:
```
/opt/kata/bin/kata-containers.io/kata-runtime list
kubectl describe pod nginx-untrusted
```

The following removes the test pod:
```
kubectl delete -f examples/nginx-untrusted.yaml
```

### Remove Kata from the Kubernetes cluster

```
kubectl delete -f kata-deploy.yaml
kubectl apply -f kata-cleanup.yaml
kubectl delete -f kata-cleanup.yaml
kubectl delete -f kata-rbac.yaml
```

## kata-deploy Details

### Dockerfile

The Dockerfile used to create the container image deployed in the DaemonSet is provided here.
This image contains all the necessary artifacts for running Kata Containers.

Host artifacts:
* kata-containers.io/kata-runtime: pulled from Kata GitHub releases page
* kata-proxy: pulled from Kata GitHub releases page
* kata-shim: pulled from Kata GitHub releases page
* qemu-system-x86_64: statically built and included in this repo, based on Kata's QEMU repo
* qemu/* : supporting binaries required for qemu-system-x86_64

Virtual Machine artifacts:
* kata-containers.img: pulled from Kata github releases page
* vmliuz.container: pulled from Kata github releases page

### Daemonsets and RBAC:

A few daemonsets are introduced for kata-deploy, as well as an RBAC to facilitate
appyling labels to the nodes.

#### runtime-labeler:

This daemonset creates a label on each node in
the cluster identifying the CRI shim in use. For example,
`kata-containers.io/container-runtime=crio` or `kata-containers.io/container-runtime=containerd.`

#### CRI-O and containerd kata installer

Depending the value of `kata-containers.io/container-runtime` label on the node, either the CRI-O or
containerd kata installation daemonset executes. These daemonsets install
the necessary kata binaries, configuration files and virtual machine artifacts on
the node. Once installed, the daemonset adds a node label `kata-containers.io/kata-runtime=true` and reconfigures
either CRI-O or containerd to make use of Kata for untrusted workloads. As a final step the daemonset
restarts either CRI-O or containerd and kubelet. Upon deletion, the daemonset removes the kata binaries
and VM artifacts and updates the node label to `kata-containers.io/kata-runtime=cleanup.`

### Kata cleanup:
This daemonset runs of the node has the label `kata-containers.io/kata-runtime=cleanup.` This daemonsets removes
the `kata-containers.io/container-runtime` and `kata-containers.io/kata-runtime` labels as well as restarts either CRI-O or containerd systemctl
daemon and kubelet. You cannot execute these restets during the preStopHook of the Kata installer daemonset,
which necessitated this final cleanup daemonset.

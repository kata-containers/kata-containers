# `kata-deploy`

* [Kubernetes quick start](#kubernetes-quick-start)
    * [Install Kata on a running Kubernetes cluster](#install-kata-on-a-running-kubernetes-cluster)
    * [Run a sample workload](#run-a-sample-workload)
    * [Remove Kata from the Kubernetes cluster](#remove-kata-from-the-kubernetes-cluster)
* [`kata-deploy` details](#kata-deploy-details)
    * [Dockerfile](#dockerfile)
    * [DaemonSets and RBAC](#daemonsets-and-rbac)
        * [Kata deploy](#kata-deploy)
        * [Kata cleanup](#kata-cleanup)

[`kata-deploy`](.) provides a Dockerfile, which contains all of the binaries
and artifacts required to run Kata Containers, as well as reference DaemonSets, which can
be utilized to install Kata Containers on a running Kubernetes cluster.

Note, installation through DaemonSets successfully installs `katacontainers.io/kata-runtime` on
a node only if it uses either containerd or CRI-O CRI-shims.

## Kubernetes quick start

### Install Kata on a running Kubernetes cluster

```sh
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy
$ kubectl apply -f kata-rbac/base/kata-rbac.yaml
$ kubectl apply -f kata-deploy/base/kata-deploy.yaml
```

or on a [k3s](https://k3s.io/) cluster:

```sh
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy
$ kubectl apply -k kata-deploy/overlays/k3s
```

### Run a sample workload

Workloads specify the runtime they'd like to utilize by setting the appropriate `runtimeClass` object within
the `Pod` specification. The `runtimeClass` examples provided define a node selector to match node label `katacontainers.io/kata-runtime:"true"`,
which will ensure the workload is only scheduled on a node that has Kata Containers installed

`runtimeClass` is a built-in type in Kubernetes. To apply each Kata Containers `runtimeClass`:
```sh
  $ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy/runtimeclasses
  $ kubectl apply -f kata-runtimeClasses.yaml
```

The following YAML snippet shows how to specify a workload should use Kata with Cloud Hypervisor:

```yaml
spec:
  template:
    spec:
      runtimeClassName: kata-clh
```

The following YAML snippet shows how to specify a workload should use Kata with Firecracker:

```yaml
spec:
  template:
    spec:
      runtimeClassName: kata-fc
```

The following YAML snippet shows how to specify a workload should use Kata with QEMU:

```yaml
spec:
  template:
    spec:
      runtimeClassName: kata-qemu
```

To run an example with `kata-qemu`:

```sh
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy/examples
$ kubectl apply -f test-deploy-kata-qemu.yaml
```

To run an example with `kata-fc`:

```sh
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy/examples
$ kubectl apply -f test-deploy-kata-fc.yaml
```

The following removes the test pods:

```sh
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy/examples
$ kubectl delete -f test-deploy-kata-qemu.yaml
$ kubectl delete -f test-deploy-kata-fc.yaml
```

### Remove Kata from the Kubernetes cluster

```sh
$ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy
$ kubectl delete -f kata-deploy/base/kata-deploy.yaml
$ kubectl apply -f kata-cleanup/base/kata-cleanup.yaml
$ kubectl delete -f kata-cleanup/base/kata-cleanup.yaml
$ kubectl delete -f kata-rbac/base/kata-rbac.yaml
$ kubectl delete -f runtimeclasses/kata-runtimeClasses.yaml
```

## `kata-deploy` details

### Dockerfile

The [Dockerfile](Dockerfile)  used to create the container image deployed in the DaemonSet is provided here.
This image contains all the necessary artifacts for running Kata Containers, all of which are pulled
from the [Kata Containers release page](https://github.com/kata-containers/runtime/releases).

Host artifacts:
* `cloud-hypervisor`, `firecracker`, `qemu-system-x86_64`, and supporting binaries
* `containerd-shim-kata-v2`
* `kata-collect-data.sh`
* `kata-runtime`

Virtual Machine artifacts:
* `kata-containers.img` and `kata-containers-initrd.img`: pulled from Kata GitHub releases page
* `vmlinuz.container` and `vmlinuz-virtiofs.container`: pulled from Kata GitHub releases page

### DaemonSets and RBAC

Two DaemonSets are introduced for `kata-deploy`, as well as an RBAC to facilitate
applying labels to the nodes.

#### Kata deploy

This DaemonSet installs the necessary Kata binaries, configuration files, and virtual machine artifacts on
the node. Once installed, the DaemonSet adds a node label `katacontainers.io/kata-runtime=true` and reconfigures
either CRI-O or containerd to register three `runtimeClasses`: `kata-clh` (for Cloud Hypervisor isolation), `kata-qemu` (for QEMU isolation),
and `kata-fc` (for Firecracker isolation). As a final step the DaemonSet restarts either CRI-O or containerd. Upon deletion,
the DaemonSet removes the Kata binaries and VM artifacts and updates the node label to `katacontainers.io/kata-runtime=cleanup`.

#### Kata cleanup

This DaemonSet runs of the node has the label `katacontainers.io/kata-runtime=cleanup`. These DaemonSets removes
the `katacontainers.io/kata-runtime` label as well as restarts either CRI-O or `containerd` `systemctl`
daemon. You cannot execute these resets during the `preStopHook` of the Kata installer DaemonSet,
which necessitated this final cleanup DaemonSet.

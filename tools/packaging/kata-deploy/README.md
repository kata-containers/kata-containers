# `kata-deploy`

[`kata-deploy`](.) provides a Dockerfile, which contains all of the binaries
and artifacts required to run Kata Containers, as well as reference DaemonSets, which can
be utilized to install Kata Containers on a running Kubernetes cluster.

> **Note**: installation through DaemonSets successfully installs `katacontainers.io/kata-runtime`
> on a node only if it uses either containerd or CRI-O CRI-shims.

## Kubernetes quick start

### Install Kata on a running Kubernetes cluster

#### k3s cluster

For your [k3s](https://k3s.io/) cluster, run:

```sh
$ git clone https://github.com/kata-containers/kata-containers.git
```

Check and switch to the stable branch of your choice, if wanted, and then run:

```bash
$ cd kata-containers/tools/packaging/kata-deploy
$ kubectl apply -f kata-rbac/base/kata-rbac.yaml
$ kubectl apply -k kata-deploy/overlays/k3s
```

#### RKE2 cluster

For your [RKE2](https://docs.rke2.io/) cluster, run:

```sh
$ git clone https://github.com/kata-containers/kata-containers.git
```

Check and switch to the stable branch of your choice, if wanted, and then run:

```bash
$ cd kata-containers/tools/packaging/kata-deploy
$ kubectl apply -f kata-rbac/base/kata-rbac.yaml
$ kubectl apply -k kata-deploy/overlays/rke2
```

#### [k0s] cluster

For your [k0s](https://k0sproject.io/) cluster, run:

```sh
$ git clone https://github.com/kata-containers/kata-containers.git
```

Check and switch to "main", and then run:

```bash
$ cd kata-containers/tools/packaging/kata-deploy
$ kubectl apply -f kata-rbac/base/kata-rbac.yaml
$ kubectl apply -k kata-deploy/overlays/k0s
```

##### Note

The supported version of k0s is **v1.27.1+k0s** and above, since the k0s support leverages a special dynamic containerd configuration mode:

> From 1.27.1 onwards k0s enables dynamic configuration on containerd CRI runtimes. This works by k0s creating a special directory in /etc/k0s/containerd.d/ where user can drop-in partial containerd configuration snippets.
> 
> k0s will automatically pick up these files and adds these in containerd configuration imports list. If k0s sees the configuration drop-ins are CRI related configurations k0s will automatically collect all these into a single file and adds that as a single import file. This is to overcome some hard limitation on containerd 1.X versions. Read more at containerd#8056

However, this would also require a magic string set in the beginning of the line for `/etc/k0s/containerd.toml`:

```
# k0s_managed=true
```

#### Vanilla Kubernetes cluster

##### Installing the latest image

The latest image refers to pre-release and release candidate content.  For stable releases, please, use the "stable" instructions.

```sh
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml
```

##### Installing the stable image

The stable image refers to the last stable releases content.

> **Note:** if you use a tagged version of the repo, the stable image does match that version.
> For instance, if you use the 2.2.1 tagged version of the kata-deploy.yaml file, then the version 2.2.1 of the kata runtime will be deployed.

```bash
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-deploy/base/kata-deploy-stable.yaml
```

#### Ensure kata-deploy is ready
```bash
$ kubectl -n kube-system wait --timeout=10m --for=condition=Ready -l name=kata-deploy pod
```

### Run a sample workload

Workloads specify the runtime they'd like to utilize by setting the appropriate `runtimeClass` object within
the `Pod` specification. The `runtimeClass` examples provided define a node selector to match node label `katacontainers.io/kata-runtime:"true"`,
which will ensure the workload is only scheduled on a node that has Kata Containers installed

`runtimeClass` is a built-in type in Kubernetes. To apply each Kata Containers `runtimeClass`:
```bash
  $ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/runtimeclasses/kata-runtimeClasses.yaml
```
The following YAML snippet shows how to specify a workload should use Kata with `Dragonball`:

```yaml
spec:
  template:
    spec:
      runtimeClassName: kata-dragonball
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

The following YAML snippet shows how to specify a workload should use Kata with StratoVirt:

```yaml
spec:
  template:
    spec:
      runtimeClassName: kata-stratovirt
```

The following YAML snippet shows how to specify a workload should use Kata with QEMU:

```yaml
spec:
  template:
    spec:
      runtimeClassName: kata-qemu
```
To run an example with `kata-dragonball`:

```bash
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/examples/test-deploy-kata-dragonball.yaml
```

To run an example with `kata-clh`:

```bash
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/examples/test-deploy-kata-clh.yaml
```

To run an example with `kata-fc`:

```bash
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/examples/test-deploy-kata-fc.yaml
```

To run an example with `kata-stratovirt`:

```bash
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/examples/test-deploy-kata-stratovirt.yaml
```

To run an example with `kata-qemu`:

```bash
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/examples/test-deploy-kata-qemu.yaml
```

The following removes the test pods:

```bash
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/examples/test-deploy-kata-dragonball.yaml
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/examples/test-deploy-kata-clh.yaml
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/examples/test-deploy-kata-fc.yaml
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/examples/test-deploy-kata-stratovirt.yaml
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/examples/test-deploy-kata-qemu.yaml
```

### Remove Kata from the Kubernetes cluster

#### Removing the latest image

```sh
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml
$ kubectl -n kube-system wait --timeout=10m --for=delete -l name=kata-deploy pod
```

After ensuring kata-deploy has been deleted, cleanup the cluster:
```sh
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml
```

The cleanup daemon-set will run a single time, cleaning up the node-label, which makes it difficult to check in an automated fashion.
This process should take, at most, 5 minutes.

After that, let's delete the cleanup daemon-set, the added RBAC and runtime classes:

```sh
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/runtimeclasses/kata-runtimeClasses.yaml
```

#### Removing the stable image

```bash
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-deploy/base/kata-deploy-stable.yaml
$ kubectl -n kube-system wait --timeout=10m --for=delete -l name=kata-deploy pod
```

After ensuring kata-deploy has been deleted, cleanup the cluster:
```bash
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup-stable.yaml
```

The cleanup daemon-set will run a single time, cleaning up the node-label, which makes it difficult to check in an automated fashion.
This process should take, at most, 5 minutes.

After that, let's delete the cleanup daemon-set, the added RBAC and runtime classes:
```bash
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup-stable.yaml
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml
$ kubectl delete -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/runtimeclasses/kata-runtimeClasses.yaml
```

## `kata-deploy` details

### Dockerfile

The [Dockerfile](Dockerfile)  used to create the container image deployed in the DaemonSet is provided here.
This image contains all the necessary artifacts for running Kata Containers, all of which are pulled
from the [Kata Containers release page](https://github.com/kata-containers/kata-containers/releases).

Host artifacts:
* `cloud-hypervisor`, `firecracker`, `qemu`, `stratovirt` and supporting binaries
* `containerd-shim-kata-v2` (go runtime and rust runtime)
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
`kata-fc` (for Firecracker isolation) and `kata-stratovirt` (for StratoVirt isolation).
As a final step the DaemonSet restarts either CRI-O or containerd. Upon deletion,
the DaemonSet removes the Kata binaries and VM artifacts and updates the node label to `katacontainers.io/kata-runtime=cleanup`.

#### Kata cleanup

This DaemonSet runs of the node has the label `katacontainers.io/kata-runtime=cleanup`. These DaemonSets removes
the `katacontainers.io/kata-runtime` label as well as restarts either CRI-O or `containerd` `systemctl`
daemon. You cannot execute these resets during the `preStopHook` of the Kata installer DaemonSet,
which necessitated this final cleanup DaemonSet.

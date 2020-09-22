# `kata-deploy`

* [Docker quick start](#docker-quick-start)
    * [Install Kata and configure Docker](#install-kata-and-configure-docker)
    * [Run a sample workload utilizing Kata containers](#run-a-sample-workload-utilizing-kata-containers)
    * [Remove Kata](#remove-kata)
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
be utilized to install Kata Containers for both Docker and on a running Kubernetes cluster.

Note, installation through DaemonSets successfully installs `katacontainers.io/kata-runtime` on
a node only if it uses either containerd or CRI-O CRI-shims.

## Docker quick start

The `kata-deploy` container image makes use of a script, `kata-deploy-docker`, for installation of
Kata artifacts and configuration of Docker to utilize the runtime. The following volumes are required to be mounted
to aid in this:
- `/opt/kata`: this is where all Kata artifacts are installed on the system
- `/var/run/dbus`, `/run/systemd`: this is require for reloading the the Docker service
- `/etc/docker`: this is required for updating `daemon.json` in order to configure the Kata runtimes in Docker


### Install Kata and configure Docker

To install:

```sh
$ docker run -v /opt/kata:/opt/kata -v /var/run/dbus:/var/run/dbus -v /run/systemd:/run/systemd -v /etc/docker:/etc/docker -it katadocker/kata-deploy kata-deploy-docker install
```

Once complete, `/etc/docker/daemon.json` is updated or created to include the Kata runtimes: `kata-qemu` and `kata-fc`, for utilizing
QEMU and Firecracker, respectively, for the VM isolation layer.

### Run a sample workload utilizing Kata containers

Run a QEMU QEMU isolated Kata container:

```sh
$ docker run --runtime=kata-qemu -itd alpine
```

Run a Firecracker isolated Kata container:

```sh
$ docker run --runtime=kata-fc -itd alpine
```

### Remove Kata

To uninstall:

```sh
$ docker run -v /opt/kata:/opt/kata -v /var/run/dbus:/var/run/dbus -v /run/systemd:/run/systemd -v /etc/docker:/etc/docker -it katadocker/kata-deploy kata-deploy-docker remove
```

After completing, the original `daemon.json`, if it existed, is restored and all Kata artifacts from `/opt/kata` are removed.

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


Workloads which utilize Kata can node-select based on `katacontainers.io/kata-runtime=true`, and are
run through an applicable runtime if they are marked with the appropriate `runtimeClass` annotation.

`runtimeClass` is a built-in type in Kubernetes versions 1.14 and greater. In Kubernetes 1.13, `runtimeClass`
is defined through a custom resource definition. For Kubernetes 1.13:
```sh
  $ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy/k8s-1.13
  $ kubectl apply -f runtimeclass-crd.yaml
```

In order to use a workload Kata with QEMU, first add a `RuntimeClass` as:
- For Kubernetes 1.14:
  ```sh
  $ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy/k8s-1.14
  $ kubectl apply -f kata-qemu-runtimeClass.yaml
  ```

- For Kubernetes 1.13:
  ```sh
  $ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy/k8s-1.13
  $ kubectl apply -f kata-qemu-runtimeClass.yaml
  ```


In order to use a workload Kata with Firecracker, first add a `RuntimeClass` as:
- For Kubernetes 1.14:
  ```sh
  $ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy/k8s-1.14
  $ kubectl apply -f kata-fc-runtimeClass.yaml
  ```

- For Kubernetes  1.13:
  ```sh
  $ cd $GOPATH/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy/k8s-1.13
  $ kubectl apply -f kata-fc-runtimeClass.yaml
  ```

The following YAML snippet shows how to specify a workload should use Kata with QEMU:

```yaml
spec:
  template:
    spec:
      runtimeClassName: kata-qemu
```

The following YAML snippet shows how to specify a workload should use Kata with Firecracker:

```yaml
spec:
  template:
    spec:
      runtimeClassName: kata-fc
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
```

## `kata-deploy` details

### Dockerfile

The [Dockerfile](Dockerfile)  used to create the container image deployed in the DaemonSet is provided here.
This image contains all the necessary artifacts for running Kata Containers, all of which are pulled
from the [Kata Containers release page](https://github.com/kata-containers/runtime/releases).

Host artifacts:
* `kata-runtime`
* `kata-fc`
* `kata-qemu`
* `kata-proxy`
* `kata-shim`
* `firecracker`
* `qemu-system-x86_64` and supporting binaries

Virtual Machine artifacts:
* `kata-containers.img`: pulled from Kata GitHub releases page
* `vmlinuz.container`: pulled from Kata GitHub releases page

### DaemonSets and RBAC

Two DaemonSets are introduced for `kata-deploy`, as well as an RBAC to facilitate
applying labels to the nodes.

#### Kata deploy

This DaemonSet installs the necessary Kata binaries, configuration files, and virtual machine artifacts on
the node. Once installed, the DaemonSet adds a node label `katacontainers.io/kata-runtime=true` and reconfigures
either CRI-O or containerd to register two `runtimeClasses`: `kata-qemu` (for QEMU isolation) and `kata-fc` (for Firecracker isolation).
As a final step the DaemonSet restarts either CRI-O or containerd. Upon deletion, the DaemonSet removes the
Kata binaries and VM artifacts and updates the node label to `katacontainers.io/kata-runtime=cleanup`.

#### Kata cleanup

This DaemonSet runs of the node has the label `katacontainers.io/kata-runtime=cleanup`. These DaemonSets removes
the `katacontainers.io/kata-runtime` label as well as restarts either CRI-O or `containerd` `systemctl`
daemon. You cannot execute these resets during the `preStopHook` of the Kata installer DaemonSet,
which necessitated this final cleanup DaemonSet.

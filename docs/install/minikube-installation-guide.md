# Installing Kata Containers in Minikube

## Introduction

[Minikube](https://kubernetes.io/docs/setup/minikube/) is an easy way to try out a Kubernetes (k8s)
cluster locally. It creates a single node Kubernetes stack in a local VM.

[Kata Containers](https://github.com/kata-containers) can be installed into a Minikube cluster using
[`kata-deploy`](../../tools/packaging/kata-deploy).

This document details the pre-requisites, installation steps, and how to check
the installation has been successful.

## Prerequisites

This installation guide has only been verified under a Minikube Linux installation, using the
[`kvm2`](https://minikube.sigs.k8s.io/docs/drivers/kvm2/) driver.

> **Notes:**
> - This installation guide may not work for macOS installations of Minikube, due to the lack of
nested virtualization support on that platform.
> - This installation guide has not been tested on a Windows installation.
> - Kata under Minikube does not currently support Kata Firecracker (`kata-fc`).
>   Although the `kata-fc` binary will be installed as part of these instructions,
>   via `kata-deploy`, pods cannot be launched with `kata-fc`, and will fail to start.

Before commencing installation, it is strongly recommended you read the
[Minikube installation guide](https://kubernetes.io/docs/tasks/tools/install-minikube/).

## Checking for nested virtualization

For Kata Containers to work under a Minikube VM, your host system must support
nested virtualization. If you are using a Linux system utilizing Intel VT-x
and the `kvm_intel` driver, you can perform the following check:

```sh
$ cat /sys/module/kvm_intel/parameters/nested
```

If your system does not report `Y` from the `nested` parameter, then details on how
to enable nested virtualization can be found on the
[KVM Nested Guests page](https://www.linux-kvm.org/page/Nested_Guests)

Alternatively, and for other architectures, the Kata Containers built in
[`check`](../../src/runtime/README.md#hardware-requirements)
command can be used *inside Minikube* once Kata has been installed, to check for compatibility.

## Setting up Minikube

To enable Kata Containers under Minikube, you need to add a few configuration options to the
default Minikube setup. You can easily accomplish this as Minikube supports them on the setup commandline.
Minikube can be set up to use either CRI-O or containerd.

Here are the features to set up a CRI-O based Minikube, and why you need them:

| what | why |
| ---- | --- |
| `--bootstrapper=kubeadm` | As recommended for [minikube CRI-O](https://minikube.sigs.k8s.io/docs/handbook/config/#runtime-configuration) |
| `--container-runtime=cri-o` | Using CRI-O for Kata |
| `--enable-default-cni` | As recommended for [minikube CRI-O](https://minikube.sigs.k8s.io/docs/handbook/config/#runtime-configuration) |
| `--memory 6144` | Allocate sufficient memory, as Kata Containers default to 1 or 2Gb |
| `--network-plugin=cni` | As recommended for [minikube CRI-O](https://minikube.sigs.k8s.io/docs/handbook/config/#runtime-configuration) |
| `--vm-driver kvm2` | The host VM driver |

To use containerd, modify the `--container-runtime` argument:

| what | why |
| ---- | --- |
| `--container-runtime=containerd` | Using containerd for Kata |

> **Notes:**
> - Adjust the `--memory 6144` line to suit your environment and requirements. Kata Containers default to
> requesting 2048MB per container. We recommended you supply more than that to the Minikube node.

The full command is therefore:

```sh
$ minikube start --vm-driver kvm2 --memory 6144 --network-plugin=cni --enable-default-cni --container-runtime=cri-o --bootstrapper=kubeadm
```

> **Note:** For Kata Containers later than v1.6.1, the now default `tcfilter` networking of Kata Containers
> does not work for Minikube versions less than v1.1.1. Please ensure you use Minikube version v1.1.1
> or above.

## Check Minikube is running

Before you install Kata Containers, check that your Minikube is operating. On your guest:


```sh
$ kubectl get nodes
```

You should see your `control-plane` node listed as being `Ready`.

Check you have virtualization enabled inside your Minikube. The following should return
a number larger than `0` if you have either of the `vmx` or `svm` nested virtualization features
available:

```sh
$ minikube ssh "grep -c -E 'vmx|svm' /proc/cpuinfo"
```

## Installing Kata Containers

You can now install the Kata Containers runtime components. You will need a local copy of some Kata
Containers components to help with this, and then use `kubectl` on the host (that Minikube has already
configured for you) to deploy them:

```sh
$ git clone https://github.com/kata-containers/kata-containers.git
$ cd kata-containers/tools/packaging/kata-deploy
$ kubectl apply -f kata-rbac/base/kata-rbac.yaml
$ kubectl apply -f kata-deploy/base/kata-deploy.yaml
```

This installs the Kata Containers components into `/opt/kata` inside the Minikube node. It can take
a few minutes for the operation to complete. You can check the installation has worked by checking
the status of the `kata-deploy` pod, which will be executing
[this script](../../tools/packaging/kata-deploy/scripts/kata-deploy.sh),
and will be executing a `sleep infinity` once it has successfully completed its work.
You can accomplish this by running the following:

```sh
$ podname=$(kubectl -n kube-system get pods -o=name | grep -F kata-deploy | sed 's?pod/??')
$ kubectl -n kube-system exec ${podname} -- ps -ef | grep -F infinity
```

> *NOTE:* This check only works for single node clusters, which is the default for Minikube.
> For multi-node clusters, the check would need to be adapted to check `kata-deploy` had
> completed on all nodes.

## Enabling Kata Containers

Now you have installed the Kata Containers components in the Minikube node. Next, you need to configure
Kubernetes `RuntimeClass` to know when to use Kata Containers to run a pod.

### Register the runtime

Now register the `kata qemu` runtime with that class. This should result in no errors:

```sh
$ cd kata-containers/tools/packaging/kata-deploy/runtimeclasses
$ kubectl apply -f kata-runtimeClasses.yaml
```

The Kata Containers installation process should be complete and enabled in the Minikube cluster.

## Testing Kata Containers

Launch a container that has been defined to run on Kata Containers. The enabling is configured by
the following lines in the YAML file. See the Kubernetes
[Runtime Class Documentation](https://kubernetes.io/docs/concepts/containers/runtime-class/#usage)
for more details.

```yaml
    spec:
      runtimeClassName: kata-qemu
```

Perform the following action to launch a Kata Containers based Apache PHP pod:

```sh
$ cd kata-containers/tools/packaging/kata-deploy/examples
$ kubectl apply -f test-deploy-kata-qemu.yaml
```

This may take a few moments if the container image needs to be pulled down into the cluster.
Check progress using:

```sh
$ kubectl rollout status deployment php-apache-kata-qemu
```

There are a couple of ways to verify it is running with Kata Containers.
In theory, you should not be able to tell your pod is running as a Kata Containers container.
Careful examination can verify your pod is in fact a Kata Containers pod.

First, look on the node for a `qemu` running. You should see a QEMU command line output here,
indicating that your pod is running inside a Kata Containers VM:

```sh
$ minikube ssh -- pgrep -a qemu
```

Another way to verify Kata Containers is running is to look in the container itself and check
which kernel is running there. For a normal software container you will be running
the same kernel as the node. For a Kata Container you will be running a Kata Containers kernel
inside the Kata Containers VM.

First, examine which kernel is running inside the Minikube node itself:

```sh
$ minikube ssh -- uname -a
```

And then compare that against the kernel that is running inside the container:

```sh
$ podname=$(kubectl get pods -o=name | grep -F php-apache-kata-qemu | sed 's?pod/??')
$ kubectl exec ${podname} -- uname -a
```

You should see the node and pod are running different kernel versions.

## Wrapping up

This guide has shown an easy way to setup Minikube with Kata Containers.
Be aware, this is only a small single node Kubernetes cluster running under a nested virtualization setup.
As such, it has limitations, but as a first introduction to Kata Containers, and how to install it under Kubernetes,
it should suffice for initial learning and experimentation.


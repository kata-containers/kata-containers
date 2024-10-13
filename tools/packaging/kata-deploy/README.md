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
$ kubectl apply -f kata-deploy/base/kata-deploy.yaml
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
$ kubectl apply -f kata-deploy/base/kata-deploy.yaml
```

#### k0s cluster

> [!IMPORTANT]  
> As in this section, when following the rest of these instructions, you must use
> `sudo k0s kubectl` instead of `kubectl` for k0s.

> [!NOTE]  
> The supported version of k0s is **v1.27.1+k0s** and above, since k0s support in Kata leverages
[dynamic runtime configuration](https://docs.k0sproject.io/v1.29.1+k0s.1/runtime/#k0s-managed-dynamic-runtime-configuration),
which was introduced in that version.
>
> Dynamic runtime configuration is enabled by default in k0s, and you can make sure it is enabled by verifying that `/etc/k0s/containerd.toml` contains the following line:
>
> ```toml
> # k0s_managed=true
> ```

For your [k0s](https://k0sproject.io/) cluster, run:

```sh
$ git clone https://github.com/kata-containers/kata-containers.git
```

Check and switch to "main", and then run:

```bash
$ cd kata-containers/tools/packaging/kata-deploy
$ sudo k0s kubectl apply -f kata-rbac/base/kata-rbac.yaml
$ sudo k0s kubectl apply -k kata-deploy/overlays/k0s
$ sudo k0s kubectl apply -f kata-deploy/base/kata-deploy.yaml
```

#### Vanilla Kubernetes cluster

```bash
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-rbac/base/kata-rbac.yaml
$ kubectl apply -f https://raw.githubusercontent.com/kata-containers/kata-containers/main/tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml
```

### Ensure Kata has been installed
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

## Development

This section is for developers who need to fix, extend and/or implement features to Kata deploy.

### Adding a new runtimeClass

Kata deploy installs and configures [Kubernetes runtime classes](https://kubernetes.io/docs/concepts/containers/runtime-class/)
matching the several Kata Containers [runtime configuration files](../../../src/runtime/config/).

The deployed runtimeClass yaml files are hosted at [`runtimeclasses`](./runtimeclasses/) directory. In particular,
[`kata-runtimeClasses.yaml`](./runtimeclasses/kata-runtimeClasses.yaml) isn't properly a runtimeClass file but rather
its content is just the concatenation of all other yaml files in the directory.

If you need to add a new runtimeClass then do:

 * Create the runtimeClass yaml file in [`runtimeclasses`](./runtimeclasses/). Follows the name convention of `kata-<MY-RUNTIME-CLASS-NAME>.yaml`.
 * Update [`kata-runtimeClasses.yaml`](./runtimeclasses/kata-runtimeClasses.yaml). Notice that the entries are sorted by runtimeClass name.
 * Update the list of `SHIMS` in [kata-deploy/base/kata-deploy.yaml](./kata-deploy/base/kata-deploy.yaml) and [kata-cleanup/base/kata-cleanup.yaml](./kata-cleanup/base/kata-cleanup.yaml)
 * (Optional) If the new runtimeClass matches a [runtime-rs configuration file](../../../src/runtime-rs/config/) then you will need to extend the runtime-rs handlers in [scripts/kata-deploy.sh](./scripts/kata-deploy.sh). As an example, you can implement the same handlers in `get_kata_containers_config_path()` and `configure_different_shims_base()` as needed for the `dragonball` runtimeClass.
 * Run the [`check-runtimeclasses.sh`](./local-build/check-runtimeclasses.sh) script ensure `kata-runtimeClasses.yaml` is correct. For example:
   ```
   $ ./tools/packaging/kata-deploy/local-build/check-runtimeclasses.sh
   ~/src/github.com/kata-containers/kata-containers/tools/packaging/kata-deploy/runtimeclasses ~/src/github.com/kata-containers/kata-containers
   ::group::Combine runtime classes
   Adding ./kata-clh.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-cloud-hypervisor.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-dragonball.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-fc.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-qemu-coco-dev.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-qemu-nvidia-gpu-snp.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-qemu-nvidia-gpu-tdx.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-qemu-nvidia-gpu.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-qemu-se.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-qemu-sev.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-qemu-snp.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-qemu-tdx.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-qemu.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-remote.yaml to the resultingRuntimeClasses.yaml
   Adding ./kata-stratovirt.yaml to the resultingRuntimeClasses.yaml
   ::endgroup::
   ::group::Displaying the content of resultingRuntimeClasses.yaml
   <OUTPUT OMITTED>
   ::endgroup::

   ::group::Displaying the content of kata-runtimeClasses.yaml
   <OUTPUT OMITTED>
   ::endgroup::

   CHECKER PASSED
   ```
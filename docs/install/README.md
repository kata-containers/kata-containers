# Kata Containers installation guide

This guide explains how to install and run [Kata Containers](https://github.com/kata-containers/kata-containers).

Starting with the 4.0.0 release, the default and recommended Kata Containers
runtime is [`runtime-rs`](../../src/runtime-rs/README.md), the Rust-based
implementation of the containerd shim v2. This guide focuses on `runtime-rs`.
Where the behaviour of the legacy Go runtime differs, it is called out
separately.

> **Note:**
>
> The Go runtime is still shipped and supported, but `runtime-rs` is the
> recommended choice for new deployments.

## Contents

- [Prerequisites](#prerequisites)
- [Choosing an installation method](#choosing-an-installation-method)
- [Install on Kubernetes (recommended)](#install-on-kubernetes-recommended)
- [Install on a Linux system](#install-on-a-linux-system)
- [Use Kata Containers with Docker](#use-kata-containers-with-docker)
- [Build and install from source](#build-and-install-from-source)
- [Verify the installation](#verify-the-installation)
- [Further information](#further-information)

## Prerequisites

### Hardware

Kata Containers runs on bare metal, or inside a VM that has nested
virtualization enabled. The following must be true on the host.

**CPU virtualization extensions** must be supported and enabled in the
firmware/BIOS:

| Architecture | Virtualization technology |
|-|-|
| `x86_64`, `amd64` | Intel VT-x (`vmx`), AMD-V (`svm`) |
| `aarch64` (`arm64`) | ARM Hyp |
| `ppc64le` | IBM Power |
| `s390x` | IBM Z & LinuxONE SIE |

On `x86_64`, confirm the extensions are exposed to the OS (`vmx` for Intel,
`svm` for AMD; no output means virtualization is unavailable):

```sh
grep -E -o '(vmx|svm)' /proc/cpuinfo | sort -u
```

**KVM** must be available: the `/dev/kvm` device has to exist and be accessible
to the user that runs the Kata shim (`root`, or a member of the `kvm` group):

```sh
ls -l /dev/kvm
```

If `/dev/kvm` is missing on `x86_64`, load the KVM module for your CPU:

```sh
sudo modprobe kvm_intel   # Intel hosts
sudo modprobe kvm_amd     # AMD hosts
```

On hosts backed by the **Microsoft Hypervisor** (Hyper-V, including nested
Linux VMs on Windows and some Azure instance types), KVM is not available and
the equivalent device is `/dev/mshv`. You then need a VMM that supports the
Microsoft Hypervisor, such as Cloud Hypervisor's mshv backend (used by the
`clh-azure` / `clh-azure-runtime-rs` runtimes):

```sh
ls -l /dev/mshv
```

**Nested virtualization** is required when the host is itself a VM: the
underlying hypervisor must expose the CPU virtualization extensions to the guest
(for example a `host-passthrough` or `host-model` CPU, with nesting enabled on
the bare-metal host).

**Host kernel modules** for `vhost` must be loaded. Kata uses VSOCK for the
communication channel between the runtime and the guest agent, which requires
`vhost_vsock` (it provides the `/dev/vhost-vsock` device); `vhost_net` is also
needed for networking:

```sh
sudo modprobe vhost_vsock
sudo modprobe vhost_net
```

Confirm the VSOCK device is present:

```sh
ls -l /dev/vhost-vsock
```

To load these modules automatically on boot, add them to a file under
`/etc/modules-load.d/`:

```sh
printf 'vhost_vsock\nvhost_net\n' | sudo tee /etc/modules-load.d/kata-containers.conf
```

These modules are required on every node that runs Kata Containers, regardless
of the installation method.

For the full list of supported platforms, see the
[hardware requirements](../../README.md#hardware-requirements).

### Software

Depending on the installation method you choose, you need one or more of the
following:

- A **Kubernetes** cluster running a supported version. See the
  [prerequisites document](../prerequisites.md) for more details.
- A **CRI-compatible container runtime** (containerd or CRI-O). containerd
  `v2.1.x` or newer is recommended; some Kata features (such as drop-in config
  merging and `multiInstallSuffix`) require containerd `v2.0` or newer.
- **Docker** (`v26+`), if you want to run Kata Containers directly with Docker.
- [`helm`](https://helm.sh/docs/intro/install/), for the recommended Kubernetes
  installation method.

## Choosing an installation method

| Method | Best for | Notes |
|-|-|-|
| [Kata Deploy Helm chart](#install-on-kubernetes-recommended) | Kubernetes | **Recommended.** Installs every required artifact and the Kata `RuntimeClass`es on each node, and handles upgrades and removal via `helm upgrade`/`helm uninstall`. |
| [Pre-built release tarball](#install-on-a-linux-system) | Docker, single nodes | Manual install; you are responsible for upgrading and removing the artifacts yourself. |
| [Build from source](#build-and-install-from-source) | Developers and contributors | Build individual components yourself. |

## Install on Kubernetes (recommended)

The [Kata Deploy Helm chart](../../tools/packaging/kata-deploy/helm-chart/README.md)
is the preferred way to install every binary and artifact required to run Kata
Containers on Kubernetes. It installs the `kata-deploy` DaemonSet, which lays
down the Kata artifacts on each node, and creates the Kata `RuntimeClass`
resources.

### Install the chart

```sh
# Pick the version you want to install, or use the latest release.
export VERSION=$(curl -sSL https://api.github.com/repos/kata-containers/kata-containers/releases/latest | jq -r .tag_name)
export CHART="oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy"

helm install kata-deploy "${CHART}" --version "${VERSION}"
```

This installs the `kata-deploy` DaemonSet and the default Kata `RuntimeClass`
resources. To see everything you can configure:

```sh
helm show values "${CHART}" --version "${VERSION}"
```

For the full set of configuration options (selecting shims, custom runtimes,
node selectors, TEE shims, drop-in configuration files and more) see the
[Helm configuration document](../helm-configuration.md).

### Select a runtime-rs RuntimeClass

The chart creates one `RuntimeClass` per enabled shim. The `runtime-rs`
based runtimes use the `-runtime-rs` suffix, and `kata-dragonball` (the built-in
Dragonball VMM) is `runtime-rs` only. For example:

| RuntimeClass | Hypervisor | Runtime |
|-|-|-|
| `kata-qemu-runtime-rs` | QEMU | runtime-rs |
| `kata-clh-runtime-rs` | Cloud Hypervisor | runtime-rs |
| `kata-dragonball` | Dragonball (built-in) | runtime-rs |
| `kata-qemu` | QEMU | Go runtime |

List the runtime classes available on your cluster:

```sh
kubectl get runtimeclasses
```

### Run a test pod

Create a pod that uses a `runtime-rs` `RuntimeClass`:

```yaml title="kata-qemu-runtime-rs-test.yaml"
apiVersion: v1
kind: Pod
metadata:
  name: kata-runtime-rs-test
spec:
  runtimeClassName: kata-qemu-runtime-rs
  containers:
    - name: test
      image: quay.io/libpod/ubuntu:latest
      command: ["uname", "-r"]
```

```sh
kubectl apply -f kata-qemu-runtime-rs-test.yaml
kubectl logs kata-runtime-rs-test
```

The kernel version printed is the Kata guest kernel, which is typically
different from the host kernel, confirming the workload is running inside a
lightweight VM.

### Uninstall

```sh
helm uninstall kata-deploy -n kube-system
```

During uninstall, Helm reports that some cluster-wide resources
(`ServiceAccount`, `ClusterRole`, `ClusterRoleBinding`) were kept due to the
resource policy. This is **normal**: a post-delete hook Job removes them so no
cluster-wide RBAC is left behind.

## Install on a Linux system

When you are not using Kubernetes (for example to run Kata Containers with
Docker), install Kata from a pre-built release tarball.

> **Note:**
>
> If your distribution packages Kata Containers, we recommend installing that
> version so it is updated when new releases are available. The instructions
> below install the newest upstream release, which is **your** responsibility to
> keep up to date.

### Download and unpack a release

Download the archive for your architecture from the
[releases page](https://github.com/kata-containers/kata-containers/releases).
Kata Containers uses [semantic versioning](https://semver.org), so install a
version that does *not* contain a dash (`-`), since that indicates a pre-release.

```sh
export VERSION=$(curl -sSL https://api.github.com/repos/kata-containers/kata-containers/releases/latest | jq -r .tag_name)
export ARCH=$(uname -m)

curl -fsSL -o kata-static.tar.zst \
  "https://github.com/kata-containers/kata-containers/releases/download/${VERSION}/kata-static-${VERSION}-${ARCH}.tar.zst"

# The archive uses an /opt/kata/ prefix.
sudo tar -xvf kata-static.tar.zst -C /
```

The release installs both runtimes side by side:

| Path | Runtime |
|-|-|
| `/opt/kata/runtime-rs/bin/containerd-shim-kata-v2` | runtime-rs (recommended) |
| `/opt/kata/bin/containerd-shim-kata-v2` | Go runtime |

The packaged configuration files live under
`/opt/kata/share/defaults/kata-containers/`. The `runtime-rs` configurations
use the `-runtime-rs` suffix (for example `configuration-qemu-runtime-rs.toml`),
and `configuration-dragonball.toml` selects the built-in Dragonball VMM.

## Use Kata Containers with Docker

Newer versions of Docker (`v26+`) can launch containers with the Kata shim
directly. Kata support with Docker is tested with QEMU as the VMM.

First, [install Kata from a release tarball](#download-and-unpack-a-release).
Then register a Kata runtime in the Docker daemon configuration. To use
`runtime-rs`, point Docker at the `runtime-rs` shim binary and select a
`runtime-rs` configuration with the `ConfigPath` option:

```json title="/etc/docker/daemon.json"
{
  "runtimes": {
    "kata": {
      "runtimeType": "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2",
      "options": {
        "ConfigPath": "/opt/kata/share/defaults/kata-containers/configuration-qemu-runtime-rs.toml"
      }
    }
  }
}
```

> **Note:**
>
> `ConfigPath` selects which configuration the shim loads (for example
> `configuration-qemu-runtime-rs.toml` for QEMU, or `configuration-dragonball.toml`
> for the built-in Dragonball VMM). If you omit it, the shim falls back to its
> default search path, where `/etc/kata-containers/configuration.toml` takes
> precedence over the packaged defaults.

Reload the Docker daemon:

```sh
sudo systemctl reload docker
```

Launch a Kata container and check the guest kernel version:

```sh
docker run --runtime kata -it --rm ubuntu:24.04 uname -r
```

> **Note:**
>
> Running `docker` *inside* a Kata Container (Docker-in-Docker) requires extra
> care. See [How to run Docker in Docker with Kata Containers](../how-to/how-to-run-docker-with-kata.md).

## Build and install from source

Developers and contributors can build Kata Containers components from source:

- [Developer Guide](../Developer-Guide.md): build and install the runtime, agent,
  guest kernel, rootfs/initrd images and hypervisors.
- [`runtime-rs` README](../../src/runtime-rs/README.md): build and install the
  Rust runtime shim, including the built-in Dragonball VMM and external
  hypervisor options.

## Verify the installation

The most reliable way to confirm Kata Containers is working is to run a workload
and check that it boots in a VM with its own guest kernel:

- On Kubernetes, run the [test pod](#run-a-test-pod) and inspect its logs.
- With Docker, run `docker run --runtime kata -it --rm ubuntu:24.04 uname -r`.

In both cases the kernel version reported from inside the container is the Kata
guest kernel, which is normally different from the host kernel (compare with
`uname -r` on the host).

## Further information

- [Helm configuration](../helm-configuration.md): advanced `kata-deploy` options
- [Prerequisites](../prerequisites.md): Kubernetes, containerd and `runc` setup
- [Upgrading](../Upgrading.md): how to upgrade an existing installation
- [Developer Guide](../Developer-Guide.md): building, debugging and troubleshooting
- [runtime-rs documentation](../../src/runtime-rs/README.md)
- [runtime documentation](../../src/runtime/README.md)
- [Limitations](../Limitations.md)

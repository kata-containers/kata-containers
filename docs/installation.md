# Installation

Kata Containers ships two runtimes side by side: the legacy Go runtime
(`src/runtime`) and `runtime-rs` (`src/runtime-rs`), the Rust implementation
of the containerd shim v2. Since the 4.0.0 release `runtime-rs` is the
default and recommended choice. The Go runtime is still shipped and supported,
but **deprecated** — see
[Go runtime deprecation](migrating-config-go-runtime-to-runtime-rs.md#go-runtime-deprecation).

The recommended install path is the [`kata-deploy` Helm chart](#install-on-kubernetes-with-helm-recommended)
on a Kubernetes cluster. Pre-built release tarballs and from-source builds are
also supported.

## Prerequisites

### Hardware

Kata Containers runs on bare metal, or inside a VM that has nested
virtualization enabled. The following must be true on every host that runs
Kata workloads, regardless of installation method.

**CPU virtualization extensions** must be supported and enabled in the
firmware/BIOS:

| Architecture | Virtualization technology |
|-|-|
| `x86_64`, `amd64` | Intel VT-x (`vmx`), AMD-V (`svm`) |
| `aarch64` (`arm64`) | ARM Hyp |
| `ppc64le` | IBM Power |
| `s390x` | IBM Z & LinuxONE SIE |

On `x86_64`, confirm the extensions are exposed to the OS (no output means
virtualization is unavailable):

```sh
grep -E -o '(vmx|svm)' /proc/cpuinfo | sort -u
```

**KVM** must be available: the `/dev/kvm` device has to exist and be
accessible to the user that runs the Kata shim (`root`, or a member of the
`kvm` group):

```sh
ls -l /dev/kvm
```

If `/dev/kvm` is missing on `x86_64`, load the KVM module for your CPU:

```sh
sudo modprobe kvm_intel   # Intel hosts
sudo modprobe kvm_amd     # AMD hosts
```

!!! tip "Microsoft Hypervisor (Hyper-V / Azure)"
    On hosts backed by the Microsoft Hypervisor — including nested Linux VMs
    on Windows and some Azure instance types — KVM is not available and the
    equivalent device is `/dev/mshv`. You then need a VMM with mshv support,
    such as Cloud Hypervisor's mshv backend used by the `clh-azure` /
    `clh-azure-runtime-rs` runtime classes.

**Nested virtualization** is required when the host is itself a VM: the
underlying hypervisor must expose the CPU virtualization extensions to the
guest (for example a `host-passthrough` or `host-model` CPU, with nesting
enabled on the bare-metal host).

**Host kernel modules** for `vhost` must be loaded. Kata uses VSOCK for the
communication channel between the runtime and the guest agent (`vhost_vsock`
provides `/dev/vhost-vsock`); `vhost_net` is needed for networking:

```sh
sudo modprobe vhost_vsock
sudo modprobe vhost_net
```

To load them automatically on boot:

```sh
printf 'vhost_vsock\nvhost_net\n' | sudo tee /etc/modules-load.d/kata-containers.conf
```

For the full list of supported platforms, see the
[hardware requirements](https://github.com/kata-containers/kata-containers/blob/main/README.md#hardware-requirements).

### Software

Which of these you need depends on the install method you pick:

- **Kubernetes ≥ v1.22** — first release where the CRI v1 API became the
  default and `RuntimeClass` left alpha. Earlier clusters need feature gates
  or CRI shims that are out of scope for this guide.
- **CRI-compatible container runtime** (containerd or CRI-O). containerd
  `v2.1.x` or newer is recommended; the `multiInstallSuffix` feature and
  drop-in config merging require containerd `v2.0+`. See
  [prerequisites](prerequisites.md) for a detailed containerd setup.
- **Kata Containers ≥ 3.12** if you install via Helm — `v3.12.0` is the
  first release that publishes the Helm chart on the releases page.
- [**`helm`**](https://helm.sh/docs/intro/install/) for the Kubernetes
  install method.
- **Docker `v26+`** if you want to run Kata Containers directly with Docker.

## Choosing an installation method

| Method | Best for | Notes |
|-|-|-|
| [Kata Deploy Helm chart](#install-on-kubernetes-with-helm-recommended) | Kubernetes | **Recommended.** Installs every required artifact and the Kata `RuntimeClass`es on each node; handles upgrades and removal via `helm upgrade` / `helm uninstall`. |
| [Pre-built release tarball](#install-from-a-pre-built-release-tarball) | Docker, single nodes | Manual install; you are responsible for upgrading and removing the artifacts yourself. |
| [Build from source](#build-and-install-from-source) | Developers and contributors | Build individual components yourself. |

## Install on Kubernetes with Helm (recommended)

[`helm`](https://helm.sh/docs/intro/install/) installs templated Kubernetes
manifests. The [Kata Deploy Helm chart](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/README.md)
lays down every Kata binary and artifact required on each node and creates
the Kata `RuntimeClass` resources.

### Install the chart

```sh
# Pick the version you want to install, or use the latest release.
export VERSION=$(curl -sSL https://api.github.com/repos/kata-containers/kata-containers/releases/latest | jq -r .tag_name)
export CHART="oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy"

helm install kata-deploy "${CHART}" --version "${VERSION}"
```

This installs Kata via short-lived, staged per-node Jobs (the default `job`
deployment mode) and the default Kata `RuntimeClass` resources on your cluster.
To see everything you can configure:

```sh
helm show values "${CHART}" --version "${VERSION}"
```

To see what versions of the chart are available:

```sh
helm show chart "${CHART}"
```

For the full set of configuration options (shim selection, custom runtimes,
node selectors, TEE shims, drop-in configuration files and more), see the
[Helm configuration document](helm-configuration.md).

!!! note "Deployment modes: Job vs DaemonSet"
    Short-lived, staged per-node Jobs (no always-on component on the node) are
    the default install model (`deploymentMode: job`). You can instead use the
    long-running `kata-deploy` DaemonSet by setting `deploymentMode: daemonset`.
    See [Deployment Modes (DaemonSet vs Job)](helm-configuration.md#deployment-modes-daemonset-vs-job)
    for details and node-selection options.

### Use a Kata RuntimeClass

The chart creates one `RuntimeClass` per enabled shim. `runtime-rs`-based
runtimes use the `-runtime-rs` suffix (for example `kata-qemu-runtime-rs`);
`kata-dragonball` (the built-in Dragonball VMM) is `runtime-rs` only. The
deprecated Go runtime is available as `kata-qemu`.

List the runtime classes available on your cluster:

```sh
kubectl get runtimeclasses
```

### Run a test pod

Schedule a pod against a `runtime-rs` `RuntimeClass` to confirm the install
works:

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

The kernel version printed is the Kata guest kernel, normally different from
the host kernel (`uname -r`) — confirming the workload ran inside a
lightweight VM.

### Uninstall

```sh
helm uninstall kata-deploy -n kube-system
```

During uninstall, Helm reports that some cluster-wide resources
(`ServiceAccount`, `ClusterRole`, `ClusterRoleBinding`) were kept due to the
resource policy. This is **normal**: a post-delete hook Job removes them so
no cluster-wide RBAC is left behind.

## Install from a pre-built release tarball

When you are not using Kubernetes — for example to run Kata Containers with
Docker — install Kata from a pre-built release tarball.

Download the archive for your architecture from the
[releases page](https://github.com/kata-containers/kata-containers/releases).

```sh
export VERSION=$(curl -sSL https://api.github.com/repos/kata-containers/kata-containers/releases/latest | jq -r .tag_name)

# Release tarballs use architecture names that differ from `uname -m`.
case "$(uname -m)" in
  x86_64)  ARCH=amd64 ;;
  aarch64) ARCH=arm64 ;;
  s390x)   ARCH=s390x ;;
  ppc64le) ARCH=ppc64le ;;
  *)
    echo "unsupported architecture: $(uname -m)" >&2
    exit 1
    ;;
esac

curl -fsSL -o kata-static.tar.zst \
  "https://github.com/kata-containers/kata-containers/releases/download/${VERSION}/kata-static-${VERSION}-${ARCH}.tar.zst"

# The archive uses an /opt/kata/ prefix.
sudo tar -xvf kata-static.tar.zst -C /
```

The release installs both runtimes side by side:

| Path | Runtime |
|-|-|
| `/opt/kata/runtime-rs/bin/containerd-shim-kata-v2` | runtime-rs (default) |
| `/opt/kata/bin/containerd-shim-kata-v2` | Go runtime (deprecated) |

Packaged configuration files live under
`/opt/kata/share/defaults/kata-containers/`. The `runtime-rs` configurations
use the `-runtime-rs` suffix (for example `configuration-qemu-runtime-rs.toml`),
and `configuration-dragonball.toml` selects the built-in Dragonball VMM.

## Use Kata Containers with Docker

Docker `v26+` can launch containers with the Kata shim directly. Kata
support with Docker is tested with QEMU as the VMM. First,
[install Kata from a release tarball](#install-from-a-pre-built-release-tarball),
then register a Kata runtime in the Docker daemon configuration. To use
`runtime-rs`, point Docker at the `runtime-rs` shim and select a `runtime-rs`
configuration with `ConfigPath`:

```json title="/etc/docker/daemon.json"
{
  "runtimes": {
    "kata": {
      "runtimeType": "/opt/kata/runtime-rs/bin/containerd-shim-kata-v2",
      "options": {
        "ConfigPath": "/opt/kata/share/defaults/kata-containers/runtime-rs/configuration-qemu-runtime-rs.toml"
      }
    }
  }
}
```

!!! note "About `ConfigPath`"
    `ConfigPath` selects which configuration the shim loads (for example
    `configuration-qemu-runtime-rs.toml` for QEMU, or
    `configuration-dragonball.toml` for the built-in Dragonball VMM). If you
    omit it, the shim falls back to its default search path, where
    `/etc/kata-containers/configuration.toml` takes precedence over the
    packaged defaults.

Restart the Docker daemon and launch a Kata container:

```sh
sudo systemctl restart docker
docker run --runtime kata -it --rm ubuntu:24.04 uname -r
```

The kernel printed is the Kata guest kernel, normally different from the
host's.

!!! warning "Docker-in-Docker"
    Running `docker` *inside* a Kata Container requires extra care. See
    [How to run Docker in Docker with Kata Containers](how-to/how-to-run-docker-with-kata.md).

## Build and install from source

Developers and contributors can build Kata components from source. See the
[Developer Guide](Developer-Guide.md) for the runtime, agent, guest kernel,
rootfs/initrd images and hypervisors, and the
[`runtime-rs` README](https://github.com/kata-containers/kata-containers/blob/main/src/runtime-rs/README.md)
for the Rust shim including the built-in Dragonball VMM and external
hypervisor options.

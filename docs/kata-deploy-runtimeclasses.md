# Kata Deploy RuntimeClass Selection

kata-deploy installs Kata Containers on Kubernetes nodes, configures the host
container runtime, and creates Kubernetes `RuntimeClass` resources. Follow the
[installation guide](installation.md#install-on-kubernetes-with-helm-recommended)
to install the Helm chart, and the [Helm configuration guide](helm-configuration.md)
to configure it.

## Choosing a RuntimeClass

A RuntimeClass selects a Kata runtime implementation, a virtual machine manager
(VMM), and a guest configuration. Several RuntimeClasses share the same shim
binary but use different VMMs, kernels, guest images, or device settings. The
chart calls these configurations `shims`; the tables below list their Kubernetes
RuntimeClass names.

For a general-purpose deployment, start with `kata-qemu-runtime-rs`. Choose a
specialized class when you need a different VMM, confidential computing, NVIDIA
GPU passthrough, or remote VMs. Since Kata Containers 4.0, the Rust runtime
(`runtime-rs`) is the default. The Go runtime remains supported but is deprecated
and receives no new features. See the
[migration guide](migrating-config-go-runtime-to-runtime-rs.md)
before moving workloads that depend on Go-specific configuration.

The names below assume `runtimeClasses.enabled: true` and no
`env.multiInstallSuffix`. With a suffix, a name such as `kata-qemu-runtime-rs`
becomes `kata-qemu-runtime-rs-<suffix>`. Architecture availability depends on the
chart and release artifacts you install: check each configuration's
`supportedArches` in the release's [values.yaml](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/values.yaml)
or the opt-in profile that defines it.
Enabling a class does not supply missing host hardware, firmware, drivers, or
external services.

### Default installation and opt-in profiles

The default chart defines the general-purpose QEMU, Cloud Hypervisor,
Dragonball, and Azure configurations. It uses the host container runtime's
existing snapshotter and does not set up an additional one. The configurations
requiring specialized storage, hardware, or an external provider are defined
in separate profiles packaged with the chart:

| Profile | RuntimeClasses it enables | Additional requirements |
| --- | --- | --- |
| [try-kata-tee.values.yaml](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-tee.values.yaml) | `kata-qemu-snp*`, `kata-qemu-tdx*`, `kata-qemu-se*`, and `kata-qemu-coco-dev*` | Nydus and guest image pulling; a matching hardware TEE for SNP, TDX, and SE. |
| [try-kata-nvidia-cpu.values.yaml](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-nvidia-cpu.values.yaml) | `kata-qemu-nvidia-cpu` and `kata-qemu-nvidia-cpu-runtime-rs` | EROFS container storage for the Rust class. |
| [try-kata-nvidia-gpu.values.yaml](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-nvidia-gpu.values.yaml) | The six `kata-qemu-nvidia-gpu*` classes | GPU passthrough setup; EROFS for the standard Rust class, nydus and a matching TEE for confidential variants. |
| [try-kata-fc.values.yaml](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-fc.values.yaml) | `kata-fc` | A devmapper snapshotter and host thin-pool configured by the operator. |
| [try-kata-remote.values.yaml](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-remote.values.yaml) | `kata-remote` | Nydus, guest image pulling, and an external peer-pods provider. |

Use the profile for the class you want; setting its `enabled` field alone does
not supply its definition or required setup. Each profile starts with
`shims.disableAll: true` and enables its own set. Choose one profile as a
starting point and edit it for your deployment. Passing multiple profiles with
`-f` merges their settings key by key and can produce a combination that neither
profile describes.

Both [`job` and `daemonset` deployment modes](helm-configuration.md#deployment-modes-daemonset-vs-job)
install all runtimes enabled by the chosen profile on supported architectures.
The mode changes how nodes are prepared and how installation is managed. The
profiles all default to `job`; using `daemonset` requires installing required host
binaries yourself and leaving `nodeBinaries` empty. See the
[profile examples](helm-configuration.md#examples) for the override and preparation
requirements, and the deployment-mode comparison for lifecycle tradeoffs.

### General-purpose VMs and alternative VMMs

These classes isolate pods in local VMs. They require host virtualization
support; ordinary VM isolation does not provide the hardware protection against
the host offered by confidential computing classes.

| RuntimeClass | Runtime / VMM | When to choose it |
| --- | --- | --- |
| `kata-qemu-runtime-rs` | Rust / QEMU | Default starting point for general-purpose VM-isolated workloads. Available across amd64, arm64, s390x, and ppc64le. |
| `kata-qemu` | Go / QEMU | Keep an existing Go-runtime deployment or configuration that has not yet migrated to Rust. |
| `kata-clh-runtime-rs` | Rust / Cloud Hypervisor | Use an external Rust VMM focused on modern cloud workloads when its device and configuration support meets your needs. |
| `kata-clh` | Go / Cloud Hypervisor | Keep workloads using Cloud Hypervisor with the Go runtime while preparing migration. |
| `kata-dragonball` | Rust / built-in Dragonball | Use Kata's integrated VMM, designed for container workloads and low startup overhead. Check its device support before replacing a QEMU configuration. There is no separate Go variant. |
| `kata-fc` | Go / Firecracker | Use a minimal microVM VMM for workloads that fit Firecracker's device model. With containerd, configure the devmapper snapshotter and its backing storage first; the chart does not set that storage up for you. |

See the [Dragonball documentation](https://github.com/kata-containers/kata-containers/blob/main/src/dragonball/README.md) and
[Firecracker setup guide](how-to/how-to-use-kata-containers-with-firecracker.md)
for details. VMMs have different device, storage, and resource-management
capabilities; evaluate your workload on the selected class before switching.

### Azure configurations

These amd64 configurations use an Azure-oriented Mariner guest image. Choose
them for a host setup that needs the corresponding Azure VMM configuration,
rather than choosing solely because the Kubernetes cluster runs on Azure.
Hosts backed by the Microsoft Hypervisor may expose `/dev/mshv` instead of
`/dev/kvm`; the VMM and host driver must support that backend. See the
[host prerequisites](installation.md#hardware).

| RuntimeClass | Runtime / VMM | When to choose it |
| --- | --- | --- |
| `kata-clh-azure-runtime-rs` | Rust / Cloud Hypervisor | Use the Azure-oriented Cloud Hypervisor configuration and Mariner guest with the Rust runtime. |
| `kata-clh-azure` | Go / Cloud Hypervisor | Keep an existing Azure-oriented Cloud Hypervisor deployment using the Go runtime. |
| `kata-openvmm-azure-runtime-rs` | Rust / OpenVMM | Use the Azure-oriented OpenVMM configuration and Mariner guest when your platform requires OpenVMM. There is no Go variant. |

The `azure` name alone does not enable a confidential guest.

### Confidential computing and development

The SNP, TDX, and SE classes select guests protected by the corresponding
hardware trusted execution environment (TEE). They require compatible CPUs,
host kernels, firmware, and platform configuration. Choose based on the TEE
available on your nodes.

| RuntimeClass | Runtime / VMM | When to choose it |
| --- | --- | --- |
| `kata-qemu-snp-runtime-rs` | Rust / QEMU | Run confidential CPU workloads on amd64 nodes with AMD SEV-SNP. |
| `kata-qemu-snp` | Go / QEMU | Keep an existing AMD SEV-SNP deployment using the Go runtime. |
| `kata-qemu-tdx-runtime-rs` | Rust / QEMU | Run confidential CPU workloads on amd64 nodes with Intel TDX. |
| `kata-qemu-tdx` | Go / QEMU | Keep an existing Intel TDX deployment using the Go runtime. |
| `kata-qemu-se-runtime-rs` | Rust / QEMU | Run confidential workloads on s390x nodes with IBM Secure Execution for Linux. |
| `kata-qemu-se` | Go / QEMU | Keep an existing IBM Secure Execution deployment using the Go runtime. |
| `kata-qemu-coco-dev-runtime-rs` | Rust / QEMU | Develop and test Confidential Containers guest functionality on an ordinary VM without a hardware TEE. |
| `kata-qemu-coco-dev` | Go / QEMU | Develop or test Confidential Containers functionality that depends on the Go runtime, without a hardware TEE. |

The `coco-dev` classes do **not** provide hardware-backed confidentiality. They
are useful for exercising the guest software on development machines, not for
validating TEE protection or hardware attestation.

Confidential computing also requires a suitable guest image-pulling and
attestation setup. These classes default to the nydus snapshotter with
containerd and guest pulling with CRI-O. A RuntimeClass alone does not configure
your attestation service, trust policy, or secret provisioning. Check
[TEE node selectors](helm-configuration.md#runtimeclass-node-selectors-for-tee-shims)
so pods land on compatible nodes; use
[try-kata-tee.values.yaml](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-tee.values.yaml)
as a deployment starting point.

### NVIDIA guest configurations

#### CPU workloads with a hardened QEMU configuration

Choose `kata-qemu-nvidia-cpu-runtime-rs` for CPU workloads that benefit from a
smaller VMM attack surface and block-based storage, compared with the
general-purpose `kata-qemu-runtime-rs`. It runs on amd64 and arm64; it does not
require an NVIDIA CPU or GPU.

If you're confused about whether you should use the
`kata-qemu-nvidia-cpu-runtime-rs`, here goes a small comparison table with
`kata-qemu-runtime-rs`. Both classes use the Rust runtime, but their packaged
VMM and guest configurations differ considerably:

| Area | `kata-qemu-runtime-rs` | `kata-qemu-nvidia-cpu-runtime-rs` |
| --- | --- | --- |
| QEMU build | General-purpose Kata QEMU build with shared-filesystem and resource-resizing support. | Minimal `qemu-no-shared-fs` build. Omits shared-filesystem devices, memory hotplug and balloon devices, DAX rootfs support, virtual IOMMU, and confidential-guest backends to reduce the exposed VMM code. |
| QEMU sandbox | QEMU seccomp sandbox is disabled by default. | Enables QEMU seccomp sandbox restrictions on obsolete operations, privilege elevation, process spawning, and resource control. |
| Guest root filesystem | Standard Kata guest image; ext4 is the default root filesystem type. | Minimal NVIDIA base guest driven by NVRC, using a compressed, read-only EROFS root filesystem with dm-verity parameters supplied by the image build. |
| Host-to-guest filesystem sharing | Uses virtio-fs by default, with a host `virtiofsd` process. | Sets `shared_fs = "none"`; container storage must use a compatible block-based path. The CPU preset selects the EROFS snapshotter. |
| Resource flexibility | QEMU build retains memory-resizing devices for configurations that use them. | Disables virtio-mem and guest memory reclamation; the minimal VMM omits memory-resizing devices. Choose it for workloads that fit that resource model. |

The [CPU preset](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-nvidia-cpu.values.yaml) configures
the Rust class to use containerd's EROFS snapshotter, with memory-backed writable
layers and dm-verity verification of read-only container image layers. Container
layers reach the guest as block devices, avoiding a shared-filesystem daemon.
The preset requires containerd 2.2 or newer and erofs-utils 1.8.2 or newer, and
disables host fs-verity to accommodate backing filesystems without that support.
See the [EROFS setup guide](how-to/how-to-use-erofs-snapshotter-with-kata.md)
for requirements and storage behavior.

The NVIDIA CPU definitions are supplied by the CPU preset, rather than the
default chart values. Use that preset or copy its complete configuration,
including `supportedArches`, `snapshotter.setup`, and
`shims.qemu-nvidia-cpu-runtime-rs.containerd.snapshotter`. The read-only guest
EROFS root filesystem and the container image EROFS snapshotter serve different
purposes.

| RuntimeClass | Runtime / VMM | When to choose it |
| --- | --- | --- |
| `kata-qemu-nvidia-cpu-runtime-rs` | Rust / minimal QEMU | Run CPU workloads with the reduced QEMU device model, seccomp sandbox, and minimal EROFS guest; use the CPU preset for EROFS container storage without virtio-fs. |
| `kata-qemu-nvidia-cpu` | Go / QEMU | Keep an existing Go-runtime deployment using the NVIDIA base guest without GPU passthrough. This class uses the general QEMU build and virtio-fs, rather than the Rust class's minimal VMM and storage configuration. |

The CPU classes provide VM isolation and hardening, without a hardware TEE.
Choose the SNP, TDX, or SE classes when you need hardware-backed confidentiality.

#### GPU workloads

The `nvidia-gpu` classes add the NVIDIA guest GPU stack and device passthrough.
They require compatible GPUs and host VFIO/IOMMU and device allocation setup.
The SNP and TDX variants additionally require compatible confidential computing
CPUs and GPUs configured for confidential computing.

| RuntimeClass | Runtime / VMM | When to choose it |
| --- | --- | --- |
| `kata-qemu-nvidia-gpu-runtime-rs` | Rust / QEMU | Run GPU-accelerated workloads with NVIDIA GPU passthrough in a standard VM. |
| `kata-qemu-nvidia-gpu` | Go / QEMU | Keep standard NVIDIA GPU passthrough workloads using the Go runtime. |
| `kata-qemu-nvidia-gpu-snp-runtime-rs` | Rust / QEMU | Run confidential GPU workloads with AMD SEV-SNP and NVIDIA GPUs in confidential computing mode. |
| `kata-qemu-nvidia-gpu-snp` | Go / QEMU | Keep an existing confidential NVIDIA GPU deployment on AMD SEV-SNP using the Go runtime. |
| `kata-qemu-nvidia-gpu-tdx-runtime-rs` | Rust / QEMU | Run confidential GPU workloads with Intel TDX and NVIDIA GPUs in confidential computing mode. |
| `kata-qemu-nvidia-gpu-tdx` | Go / QEMU | Keep an existing confidential NVIDIA GPU deployment on Intel TDX using the Go runtime. |

The standard GPU classes select nodes labeled `nvidia.com/cc.ready.state: "false"`;
the confidential GPU classes select `"true"` together with the corresponding
SNP or TDX label. Ensure the GPU Operator and Node Feature Discovery, or your
own labeling process, advertise the appropriate state. See the
[GPU passthrough guide](use-cases/NVIDIA-GPU-passthrough-and-Kata-QEMU.md)
and [NVIDIA guest settings](helm-configuration.md#nvidia-guest-settings).
The [GPU preset](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-nvidia-gpu.values.yaml) provides
a deployment example.

### Remote VMs

| RuntimeClass | Runtime / VMM | When to choose it |
| --- | --- | --- |
| `kata-remote` | Go / remote hypervisor service | Run pods in VMs provisioned outside the Kubernetes worker through a peer-pods remote hypervisor service, rather than starting a local VMM. |

`remote` is defined in [try-kata-remote.values.yaml](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-remote.values.yaml),
not in the default chart values. The profile installs the local shim and sets
up nydus and guest pulling, but does not deploy the external service or provision
its VM images, credentials, networking, or attestation setup. Its configuration expects a remote hypervisor
socket, by default `/run/peerpod/hypervisor.sock`, and guest-side image pulling.
See the [remote configuration template](https://github.com/kata-containers/kata-containers/blob/main/src/runtime/config/configuration-remote.toml.in)
for the local runtime settings.

### Debug and devkit variants

The chart can also create diagnostic variants of each enabled class:

| RuntimeClass pattern | Enable with | When to choose it |
| --- | --- | --- |
| `kata-<configuration>-debug` | `debug: true` | Troubleshoot a pod with guest debug logging and the agent debug console, for example `kata-qemu-runtime-rs-debug`. |
| `kata-<configuration>-devkit` | `debug: true` and `devkit: true` | Inspect the guest using the devkit extension's Ubuntu shell, package manager, and debugging tools, for example `kata-qemu-runtime-rs-devkit`. |

Both variants inherit the base class's VMM, scheduling selectors, and pod
overhead. Guest debug settings apply to the diagnostic classes; `debug: true`
also enables host debug logging. With `env.multiInstallSuffix`, the names are
`kata-<configuration>-<suffix>-debug` and
`kata-<configuration>-<suffix>-devkit`. Debug settings and guest extensions
can change guest measurements, so account for them when testing attestation.
Keep devkit disabled in production and confidential computing deployments.

## Enable and use the selected RuntimeClasses

For configurations defined in the default values, `shims.disableAll: false`
enables entries whose `enabled` setting is null, subject to their architecture
restrictions. To install only the default configurations you need, set
`disableAll: true` and explicitly enable them. Remove the `kata-` prefix from
the RuntimeClass name to find its `shims` key:

```yaml
# selected-runtimes.values.yaml
shims:
  disableAll: true
  qemu-runtime-rs:
    enabled: true
```

Pass this file to `helm install` or `helm upgrade` with
`-f selected-runtimes.values.yaml`, as described in the
[installation guide](installation.md#install-on-kubernetes-with-helm-recommended).
For a specialized class, obtain the matching profile from the same chart
version and use it instead. For example, to install the NVIDIA CPU classes:

```sh
helm pull oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy \
  --version VERSION --untar
helm install kata-deploy ./kata-deploy \
  -f kata-deploy/try-kata-nvidia-cpu.values.yaml
```

Edit the profile's `shims` entries if you want only one of its classes. A class
absent from the default values needs its complete profile definition, including
`supportedArches`: the chart rejects enabled entries without that field.

List the classes created on your cluster with:

```sh
kubectl get runtimeclasses
```

Choose a class explicitly in the pod specification:

```yaml
spec:
  runtimeClassName: kata-qemu-runtime-rs
```

`defaultShim` selects which configuration the optional default `kata`
RuntimeClass uses; it does not choose the runtime for pods that omit
`runtimeClassName`. Creating that alias requires `runtimeClasses.createDefault:
true`, and its selected configuration must be enabled for the node architecture.
See [defaultShim](helm-configuration.md#defaultshim) and
[custom runtimes](helm-configuration.md#custom-runtimes) for aliases
and configurations derived from these built-in classes.

# Enabling NVIDIA GPU workloads using GPU passthrough with Kata Containers

This page provides:

1. A description of the components involved when running GPU workloads with
   Kata Containers using the NVIDIA TEE and non-TEE GPU runtime classes.
1. An explanation of the orchestration flow on a Kubernetes node for this
   scenario.
1. A deployment guide to utilize these runtime classes.

The goal is to educate readers familiar with Kubernetes and Kata Containers
on NVIDIA's reference implementation as reflected in Kata CI's build and test
framework. This guide uses the same KRAB deployment flow as Kata CI. Readers
can use this stack or apply its principles to their own Kata Containers
deployment.

We assume the reader is familiar with Kubernetes, Kata Containers, and
Confidential Containers.

!!! note
    The currently supported modes for enabling GPU workloads in the TEE
    scenario are: (1) single‑GPU passthrough (one physical GPU per pod) and
    (2) multi-GPU passthrough on NVSwitch (NVLink) based HGX systems
    (for example, HGX Hopper (SXM) and HGX Blackwell / HGX B200).

## Component Overview

Before providing deployment guidance, we describe the components involved to
support running GPU workloads. We start from a top to bottom perspective,
from host device provisioning and allocation via the Kata runtime to the
components within the NVIDIA GPU Utility Virtual Machine (UVM) root filesystem.

### Device provisioning and allocation

The host-side components have separate responsibilities:

- [kata-device-provisioner](https://github.com/kata-containers/kata-device-provisioner)
  configures the GPU confidential computing (CC) mode, binds passthrough devices
  to their VFIO drivers, and writes persistent boot configuration. Its chart
  uses short-lived per-node Jobs and labels nodes after successful provisioning.
- [kata-device-plugin](https://github.com/kata-containers/kata-device-plugin)
  discovers VFIO-bound devices, creates host CDI specifications, and advertises
  them to Kubelet. It runs as a read-only DaemonSet and does not change GPU mode
  or driver bindings. By default, GPUs are advertised as `nvidia.com/gpu` and
  NVSwitches as `nvidia.com/nvswitch`.

Once provisioning and runtime installation complete, a workload requests a
GPU through its resource limits and selects the matching RuntimeClass:

```yaml
apiVersion: v1
kind: Pod
...
spec:
  ...
  runtimeClassName: kata-qemu-nvidia-gpu-snp-runtime-rs
  ...
    resources:
      limits:
        nvidia.com/gpu: 1
...
```

Kubelet calls the device plugin to allocate the requested GPU. The plugin
returns CDI device names, such as `nvidia.com/gpu=0`. Containerd resolves those
names against the host CDI registry when preparing the container's OCI spec.
For example, `/var/run/cdi/kata.nvidia.com-gpu.yaml` maps GPU indices to host
IOMMUFD device paths under `/dev/vfio/devices/`.

The device plugin can also advertise hardware-specific resource names with
its `resourceNaming: sku` chart setting. In that mode, use the resource names
reported by the node in workload limits instead of `nvidia.com/gpu`.

### Kata runtime

The Kata runtime for the NVIDIA GPU handlers is configured to cold-plug VFIO
devices (`cold_plug_vfio` is set to `root-port`). Cold-plug is by design the
only supported mode for NVIDIA GPU passthrough of the NVIDIA reference stack.

With cold-plug, the Kata runtime attaches the GPU at VM launch time, when
creating the pod sandbox. This happens *before* the create container request,
i.e., before the Kata runtime receives the OCI spec including device
configurations from containerd. Thus, a mechanism to acquire the device
information is required. The runtime queries Kubelet's Pod Resources API to
discover allocated GPU device IDs. It resolves these through the host CDI
specifications to determine the device path the GPU is backed by
(e.g., `/dev/vfio/devices/vfio0`). Finally, the runtime resolves the device's
PCI BDF (e.g., `0000:21:00.0`) and cold-plugs the GPU by launching QEMU with
relevant parameters for device passthrough (e.g.,
`-device vfio-pci,host=0000:21:00.0,x-pci-vendor-id=0x10de,x-pci-device-id=0x2321,bus=rp0,iommufd=iommufdvfio-faf829f2ea7aec330`).

The runtime also creates *inner runtime* CDI annotations which map host VFIO
devices to guest GPU devices. These annotations tell the kata-agent, here
referred to as the inner runtime (inside the UVM), which passthrough devices
belong to each container.
The annotations are key-value pairs consisting of `cdi.k8s.io/vfio<num>` keys
(derived from the host VFIO device path, e.g., `/dev/vfio/devices/vfio1`) and
`nvidia.com/gpu=<index>` values (referencing the corresponding device in the
guest CDI spec). Host and guest device indices need not match. These
annotations are injected by the runtime during container creation.

We continue describing the orchestration flow inside the UVM in the next
section.

### Kata NVIDIA GPU UVM

#### UVM composition

To better understand the orchestration flow inside the NVIDIA GPU UVM, we
first look at the components its root filesystem contains. Should you decide
to use your own root filesystem to enable NVIDIA GPU scenarios, this should
give you a good idea on what ingredients you need.

From a file system perspective, the UVM is composed of two files: a standard
Kata kernel image and the NVIDIA GPU rootfs in initrd or disk image format.
These two files are being utilized for the QEMU launch command when the UVM
is created.

The two most important pieces in Kata Container's build recipes for the
NVIDIA GPU root filesystem are the `nvidia_chroot.sh` and `nvidia_rootfs.sh`
files. The build follows a two-stage process. In the first stage, a
full-fledged Ubuntu-based root filesystem is composed within a chroot
environment. In this stage, NVIDIA kernel modules are built and signed
against the current Kata kernel and relevant NVIDIA packages are installed.
In the second stage, a chiseled build is performed: Only relevant contents
from the first stage are copied and compressed into a new distro-less root
filesystem folder. Kata's build infrastructure then turns this root
filesystem into the NVIDIA initrd and image files.

The resulting root filesystem contains the following software components:

- NVRC - the
  [NVIDIA Runtime Container init system](https://github.com/NVIDIA/nvrc/tree/main)
- NVIDIA drivers (kernel modules)
- NVIDIA user space driver libraries
- NVIDIA user space tools
- kata-agent
- confidential computing guest components: the attestation agent,
  confidential data hub and api-server-rest binaries
- CRI-O pause container (for the guest image-pull method)
- BusyBox utilities (provides a base set of libraries and binaries, and a
  linker)
- some supporting files, such as file containing a list of supported GPU
  device IDs which NVRC reads

#### UVM orchestration flow

When the Kata runtime asks QEMU to launch the VM, the UVM's Linux kernel
boots and mounts the root filesystem. After this, NVRC starts as the initial
process.

NVRC scans for NVIDIA GPUs on the PCI bus, loads the
NVIDIA kernel modules, waits for driver initialization, creates the device nodes,
and initializes the GPU hardware (using the `nvidia-smi` binary). NVRC also
creates the guest-side CDI specification file (using the
`nvidia-ctk cdi generate` command). This file specifies devices of
`kind: nvidia.com/gpu`, i.e., GPUs appearing to be physical GPUs on regular
bare metal systems. The guest CDI specification also contains `containerEdits`
for each device, specifying device nodes (e.g., `/dev/nvidia0`,
`/dev/nvidiactl`), library mounts, and environment variables to be mounted
into the container which receives the passthrough GPU.

Then, NVRC forks the Kata agent while continuing to run as the
init system. This allows NVRC to handle ongoing GPU management tasks
while kata-agent focuses on container lifecycle management. See the
[NVRC sources](https://github.com/NVIDIA/nvrc/blob/main/src/main.rs) for an
overview on the steps carried out by NVRC.

When the Kata runtime sends the create container request, the Kata agent
parses the inner runtime CDI annotation. For example, for the inner runtime
annotation `"cdi.k8s.io/vfio1": "nvidia.com/gpu=0"`, the agent looks up device
`0` in the guest CDI specification with `kind: nvidia.com/gpu`.

The Kata agent also reads the guest CDI specification's `containerEdits`
section and injects relevant contents into the OCI spec of the respective
container. The kata agent then creates and starts a `rustjail` container
based on the final OCI spec. The container now has relevant device nodes,
binaries and low-level libraries available, and can start a user application
linked against the CUDA runtime API (e.g., `libcudart.so` and other
libraries). When used, the CUDA runtime API in turn calls the CUDA driver
API and kernel drivers, interacting with the pass-through GPU device.

An additional step is exercised in our CI samples: when using images from an
authenticated registry, the guest-pull mechanism triggers attestation using
Trustee's Key Broker Service (KBS) for secure release of the NGC API
authentication key used to access the NVCR container registry.
In this flow the CPU and all additional devices are attested.
GPUs will automatically be set to ready by NVRC per the NVRC configuration
flag in the default kernel command line.

## Deployment Guidance

This guidance assumes you use bare-metal machines with proper support for
Kata's non-TEE and TEE GPU workload deployment scenarios for your Kubernetes
nodes. Use [KRAB](https://github.com/kata-containers/krab), the Kata Reference
Architecture Builder, to configure and deploy the stack. Confidential runtime
profiles use guest image pulling; the non-confidential Rust GPU profile uses
the EROFS snapshotter. The sections below also cover a CUDA vectorAdd sample,
agent security policies, and attestation integration.

A similar deployment guide and scenario description can be found in NVIDIA resources
under
[NVIDIA Confidential Containers Reference Architecture](https://docs.nvidia.com/confidential-computing/containers/overview).

### Feature Set

The NVIDIA stack for Kata Containers leverages features for the confidential
computing scenario from both the confidential containers open source project
and from the Kata Containers source tree, such as:
- composite attestation using Trustee and the NVIDIA Remote Attestation
Service NRAS
- generating kata agent security policies using the genpolicy tool
- use of signed sealed secrets
- access to authenticated registries for container image guest-pull
- container image signature verification and encrypted container images
- ephemeral container data and image layer storage

For the use of these features, we refer to separate documentation in the
kata-containers and confidential-containers documentation resources.
For example, see a
[list of features](https://confidentialcontainers.org/docs/features/) along
with their documentation in the confidential-containers documentation.

### Requirements

The requirements for the TEE scenario are:

- Ubuntu 26.04 as host OS
- CPU with AMD SEV-SNP or Intel TDX support with proper BIOS/UEFI version
and settings
- CC-capable Hopper/Blackwell GPU with proper VBIOS version.

BIOS and VBIOS configuration is out of scope for this guide. Other resources,
such as the documentation found on the
[NVIDIA Trusted Computing Solutions](https://docs.nvidia.com/nvtrust/index.html)
page, on the
[Secure AI Compatibility Matrix](https://www.nvidia.com/en-us/data-center/solutions/confidential-computing/secure-ai-compatibility-matrix/)
page, and on the above linked NVIDIA documentation, provide guidance on
selecting proper hardware and on properly configuring its firmware and OS.

### Installation

#### Containerd and Kubernetes

First, set up your Kubernetes cluster. For instance, in Kata CI, our NVIDIA
jobs use a single-node vanilla Kubernetes cluster with containerd v2.3
and Kata's current supported Kubernetes version. This cluster is
being set up using the `deploy_k8s` function from the script file
`tests/integration/kubernetes/gha-run.sh`. If you intend to run this script,
follow these steps, and make sure you have `yq` and `helm` installed. Note
that, these scripts query the GitHub API, so creating and declaring a
personal access token prevents rate limiting issues.
You can execute the function as follows:
```bash
$ export GH_TOKEN="<your-gh-pat>"
$ export KUBERNETES="vanilla"
$ export CONTAINER_ENGINE="containerd"
$ export CONTAINER_ENGINE_VERSION="v2.3"
$ source tests/gha-run-k8s-common.sh
$ deploy_k8s
```

!!! note
    We recommend configuring Kubelet's `runtimeRequestTimeout` to `20m` instead
    of its two-minute default.
    Using the guest-pull mechanism, pulling large images may take a significant
    amount of time and may delay container start, possibly leading your Kubelet
    to de-allocate your pod before it transitions from the *container creating*
    to the *container running* state. The NVIDIA shim configurations use a
    `create_container_timeout` of 1200s, which is the equivalent value on shim
    side, controlling the time the shim allows for a container to remain in
    *container creating* state.
    If you need a timeout of more than 1200s, you will also need to adjust the
    agent's `image_pull_timeout`, which in turn sets the confidential data
    hub's image pull API timeout in seconds. For this, add the
    `agent.image_pull_timeout=<seconds>` kernel parameter to your shim
    configuration's `kernel_params` field, or pass the parameter explicitly
    via the `io.katacontainers.config.hypervisor.kernel_params: "..."` pod
    annotation. The default value for this timeout is 1200s.

!!! note
    The NVIDIA GPU runtime classes use VFIO cold-plug which, as
    described above, requires the Kata runtime to query Kubelet's Pod Resources
    API to discover allocated GPU devices during sandbox creation. For
    Kubernetes versions **older than 1.34**, you must explicitly enable the
    `KubeletPodResourcesGet` feature gate in your Kubelet configuration. For
    Kubernetes 1.34 and later, this feature is enabled by default. You must also
    explicitly enable the `RuntimeClassInImageCriApi` feature gate, which is
    required when using multiple snapshotters side by side and is disabled by
    default.

#### Deploy the GPU stack with KRAB

Install the CLI from the [KRAB downloads](https://kata-containers.github.io/krab/#cli)
on the machine where you run `kubectl` and Helm. Use the terminal editor to
configure a single release for the GPU stack:

```bash
$ krab
```

Select your Kubernetes distribution, node architecture, GPU hardware, CC mode,
and CPU TEE. Choose `off` for non-confidential passthrough, `on` for supported
single-GPU confidential workloads and Blackwell multi-GPU workloads, or
`ppcie` for supported Hopper multi-GPU workloads. KRAB offers compatible CPU
TEEs and selects the required RuntimeClasses from its compatibility catalog.

Keep automatic component selection enabled. KRAB includes kata-deploy,
kata-device-provisioner, kata-device-plugin, and Node Feature Discovery (NFD),
plus QGS for Intel TDX. If the cluster already has NFD, disable KRAB's NFD
component in **Advanced** so that only one release owns it. Review the generated
node selectors for the provisioner profiles, device plugin, and Kata
installation to ensure they target only the intended GPU nodes. Use the
**Advanced** chart overrides if you need to narrow those selectors. Save the
configuration as `krab.yaml`.

Run **Node pre-check** from the editor for the selected GPU nodes before
installing. Enable an IOMMU and use a kernel that exposes VFIO character
devices and IOMMUFD. The host must have `kmod` installed. The pre-check
establishes host prerequisites; validate workload launch and attestation
separately.

Stop GPU workloads before provisioning or changing mode: provisioning can
reset GPUs and replaces their host driver bindings. When migrating an
existing deployment, remove the previous GPU managers and device plugin
from these nodes before provisioning.

Validate the saved configuration and review the generated values and Helm
execution plan. Replace `my-cluster` with your Kubernetes context:

```bash
$ krab validate --config krab.yaml
$ krab generate --config krab.yaml --output values.yaml
$ krab install --config krab.yaml --context my-cluster
```

After reviewing the selected nodes, GPU modes, RuntimeClasses, and components,
install the release:

```bash
$ krab install --config krab.yaml --context my-cluster --execute
```

Wait for device provisioning and Kata installation to succeed before running
GPU workloads, as described in [Cluster validation and preparation](#cluster-validation-and-preparation).
For other KRAB workflows and advanced configuration, see the
[KRAB documentation](https://kata-containers.github.io/krab/#docs).
Components can also be installed individually; see the deployment instructions
for [kata-device-provisioner](https://github.com/kata-containers/kata-device-provisioner#deploying-it),
[kata-device-plugin](https://github.com/kata-containers/kata-device-plugin#deploying),
and [kata-deploy](../../tools/packaging/kata-deploy/helm-chart/README.md).

!!! note
    QEMU seccomp sandboxing is enabled by default for the NVIDIA QEMU runtime
    classes. See [Using seccomp with runtime-rs](../how-to/how-to-use-seccomp-with-runtime-rs.md)
    for configuration details.

For node lifecycle management, see the
[lifecycle-manager](https://github.com/kata-containers/lifecycle-manager)
repository, which enables Argo Workflows-based lifecycle management of Kata
node deployments.

#### Trustee's KBS for remote attestation

For our Kata CI runners we use Trustee's KBS for composite attestation for
secure key release, for instance, for test scenarios which use authenticated
container images. In such scenarios, the credentials to access the
authenticated container registry are only released to the confidential guest
after successful attestation. Please see the section below for more
information about this.

```bash
$ export NVIDIA_VERIFIER_MODE="remote"
$ export KBS_INGRESS="nodeport"
$ bash tests/integration/kubernetes/gha-run.sh deploy-coco-kbs
$ bash tests/integration/kubernetes/gha-run.sh install-kbs-client
```

Please note, that Trustee can also be deployed via any other upstream
mechanism as documented by the
[confidential-containers repository](https://github.com/confidential-containers/trustee).
For our architecture it is important to set up KBS in the remote verifier
mode which requires entering a licensing agreement with NVIDIA, see the
[notes in confidential-containers repository](https://github.com/confidential-containers/trustee/blob/main/deps/verifier/src/nvidia/README.md).

### Cluster validation and preparation

Check that the provisioning and Kata installation Jobs completed successfully,
and that the device plugin and NFD pods are running:

```bash
kubectl --context my-cluster get jobs,pods -n kata-system
kubectl --context my-cluster get runtimeclasses
kubectl --context my-cluster get nodes -L nvidia.com/cc.ready.state
kubectl --context my-cluster get node <gpu-node> -o jsonpath='{.status.allocatable}'
```

Expect `nvidia.com/gpu` and, where present, `nvidia.com/nvswitch` in the node's
allocatable resources when using default resource naming. A successful
provisioning run sets `nvidia.com/cc.ready.state` to `false` for `off`, and
`true` for confidential modes. Check the Job result as well as labels: a failed
provisioning run can leave labels from a previous successful run.

On the selected node, verify that the devices are bound to their VFIO driver
and that VFIO character devices are present:

```bash
lspci -nnk -d 10de:
ls /dev/vfio/devices/
```

Use the same Kubernetes context for the sample commands below.

### Run the CUDA vectorAdd sample

Create the pod manifest with:

```bash
$ cat > cuda-vectoradd-kata.yaml.in << 'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: cuda-vectoradd-kata
  namespace: default
spec:
  runtimeClassName: ${GPU_RUNTIME_CLASS_NAME}
  restartPolicy: Never
  containers:
  - name: cuda-vectoradd
    image: "nvcr.io/nvidia/k8s/cuda-sample:vectoradd-cuda12.5.0-ubuntu22.04"
    resources:
      limits:
        nvidia.com/gpu: "1"
        memory: 16Gi
EOF
```

Depending on your scenario and on the CC state, define the environment
variable with your desired runtime class name:

```bash
$ export GPU_RUNTIME_CLASS_NAME="kata-qemu-nvidia-gpu-snp-runtime-rs"
```

Then, deploy the sample Kubernetes pod manifest and observe the pod logs:

```bash
$ envsubst < ./cuda-vectoradd-kata.yaml.in | kubectl apply -f -
$ kubectl wait --for=jsonpath='{.status.phase}'=Succeeded pod/cuda-vectoradd-kata --timeout=60s
$ kubectl logs -n default cuda-vectoradd-kata
```

Expect the following output:

```text
[Vector addition of 50000 elements]
Copy input data from the host memory to the CUDA device
CUDA kernel launch with 196 blocks of 256 threads
Copy output data from the CUDA device to the host memory
Test PASSED
Done
```

To stop the pod, run: `kubectl delete pod cuda-vectoradd-kata`.

### Next steps

#### Use multi-GPU passthrough

If you have machines supporting multi-GPU passthrough, use a pod deployment
manifest which requests `nvidia.com/gpu: 8` and `nvidia.com/nvswitch: 4`
for a board with eight GPUs and four NVSwitches.
On the NVIDIA Hopper architecture multi-GPU passthrough uses protected PCIe
(PPCIE) which claims exclusive use of the nvswitches for a single CVM. In
this case, transition your relevant node(s) GPU mode to `ppcie` mode.
The NVIDIA Blackwell architecture uses NVLink encryption which places the
switches outside of the Trusted Computing Base (TCB) and so does not
require a separate switch setting.

#### Transition between CC and non-CC mode

Stop GPU workloads, open `krab.yaml` in the terminal editor, and change the
GPU mode. KRAB updates the required RuntimeClasses for the new selection.
Save, validate, and review the updated values and plan before upgrading the
same release:

```bash
$ krab edit --config krab.yaml
$ krab validate --config krab.yaml
$ krab generate --config krab.yaml --output values.yaml --force
$ krab install --config krab.yaml --context my-cluster
$ krab install --config krab.yaml --context my-cluster --execute
```

Changing a node label alone does not trigger a mode change: there is no
long-running CC manager watching labels.

Wait for successful provisioning and check device allocation before starting
workloads again. Use `kata-qemu-nvidia-gpu-runtime-rs` for `off`, or the
matching SNP/TDX RuntimeClass for a confidential mode. The runtime must match
both the GPU mode and the node's CPU TEE.

#### Deploy pods with Kata agent security policies

With GPU passthrough being supported by the
[genpolicy tool](https://github.com/kata-containers/kata-containers/tree/main/src/tools/genpolicy),
you can use the tool to create a Kata agent security policy. Our CI deploys
all sample pod manifests with a Kata agent security policy.

The `genpolicy` binary is shipped in the separate Kata tools archive and is
not installed by the Kata Containers Helm chart. For evaluation on the
single-node cluster described in this guide, install the Kata tools archive
matching the installed Kata release. Set `VERSION` to that Kata
release tag (not the KRAB chart version). For example, run the
following commands from the root of the Kata Containers source tree:

```bash
$ mkdir -p kata-tools-artifacts
$ curl -fsSL \
    "https://github.com/kata-containers/kata-containers/releases/download/${VERSION}/kata-tools-static-${VERSION}-amd64.tar.zst" \
    --output kata-tools-artifacts/kata-tools-static.tar.zst
$ bash tests/integration/kubernetes/gha-run.sh install-kata-tools
$ test -x /opt/kata/bin/genpolicy
$ rm -rf kata-tools-artifacts
```

These commands install `genpolicy` and its settings under `/opt/kata` on the
same host that runs this single-node Kubernetes instance. This is intended
for evaluation purposes and for reproducing Kata CI locally.

Note that, in Kata CI, we use snippets such as the following to modify the
genpolicy default settings:
```bash
[
  {
    "op": "replace",
    "path": "/kata_config/oci_version",
    "value": "1.3.0"
  }
]
```
This modification is applied via the genpolicy drop-in configuration file
`src/tools/genpolicy/drop-in-examples/20-oci-1.3.0-drop-in.json`.
When using a newer (or older) containerd version, the OCI version field
may need to be adjusted accordingly.

#### Run Kata CI tests locally

The NVIDIA CI suite covers CUDA vectorAdd, NUMA, DCGM, composite attestation,
vLLM, and other GPU scenarios. See the
[NVIDIA CI workflow](../../.github/workflows/run-k8s-tests-on-nvidia-gpu.yaml)
and [test runner](../../tests/integration/kubernetes/run_kubernetes_nv_tests.sh)
for the current test selection and setup. KBS is required for attestation
and authenticated guest image pulls, but not for the CUDA vectorAdd test.

Run on the single-node GPU host configured above, using a Kata Containers
source tree checked out at the tag matching the installed Kata release.
Install `genpolicy` and its settings as described in
[Deploy pods with Kata agent security policies](#deploy-pods-with-kata-agent-security-policies)
to keep the runtime, policy generator, and tests at the same version. The
[test prerequisites](../../tests/README.md) also apply.

For example, from the repository root, run CUDA vectorAdd with the SNP Rust
runtime selected earlier:

```bash
$ export KATA_HYPERVISOR=qemu-nvidia-gpu-snp-runtime-rs
$ export AUTO_GENERATE_POLICY=yes
$ bash tests/integration/kubernetes/gha-run.sh install-bats
$ K8S_TEST_NV="k8s-nvidia-cuda.bats" bash tests/integration/kubernetes/gha-run.sh run-nv-tests
```

Set `KATA_HYPERVISOR` to match your installed runtime for other GPU modes or
CPU TEEs. The test helpers select `kata-${KATA_HYPERVISOR}` automatically.
Use `K8S_TEST_NV` to choose other test files. Before running tests that use
attestation, configure KBS as described below and allow the `kernel_params`
hypervisor annotation in the test runtime configuration, as the CI workflow
does. NIM tests also require a valid NGC API key in `NGC_API_KEY`.

#### Deploy pods using attestation

Attestation is a fundamental piece of the confidential containers solution.
In our upstream CI we use attestation at the example of leveraging the
authenticated container image pull mechanism where container images reside
in the authenticated NVCR registry (`k8s-nvidia-nim.bats`), and for
requesting secrets from KBS (`k8s-confidential-attestation.bats`). KBS will
release the image pull secret to a confidential guest. To get the
authentication credentials from inside the guest, KBS must already be
deployed and configured. In our CI samples, we configure KBS with the guest
image pull secret, a resource policy, and launch the pod with certain kernel
command line parameters:
`"agent.image_registry_auth=kbs:///default/credentials/nvcr agent.aa_kbc_params=cc_kbc::${CC_KBS_ADDR}"`.

The `agent.aa_kbc_params` option is a general configuration for attestation.
For your use case, you need to set the IP address and port under which KBS
is reachable through the `CC_KBS_ADDR` variable (see our CI sample). This
tells the guest how to reach KBS. Something like this must be set whenever
attestation is used, but on its own this parameter does not trigger
attestation. The `agent.image_registry_auth` option tells the guest to ask
for a resource from KBS and use it as the authentication configuration. When
this is set, the guest will request this resource at boot (and trigger
attestation) regardless of which image is being pulled.

To deploy your own pods using authenticated container images, or secure key
release for attestation, follow steps similar to our mentioned CI samples.

#### Deploy pods using your own containers and manifests

You can author pod manifests leveraging your own containers, for instance,
containers built using the CUDA container toolkit. We recommend to start
with a CUDA base container.

When using the GPU runtime classes, the GPUs will automatically be set to ready.

!!! note
    - musl-based container images (e.g., using Alpine), or distro-less
      containers are not supported.

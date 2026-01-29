# Enabling NVIDIA GPU workloads using GPU passthrough with Kata Containers

This page provides:
1. A description of the components involved when running GPU workloads with
   Kata Containers using the NVIDIA TEE and non-TEE GPU runtime classes.
1. An explanation of the orchestration flow on a Kubernetes node for this
   scenario.
1. A deployment guide enabling to utilize these runtime classes.

The goal is to educate readers familiar with Kubernetes and Kata Containers
on NVIDIA's reference implementation which is reflected in Kata CI's build
and test framework. With this, we aim to enable readers to leverage this
stack, or to use the principles behind this stack in order to run GPU
workloads on their variant of the Kata Containers stack.

We assume the reader is familiar with Kubernetes, Kata Containers, and
Confidential Containers.

> **Note:**
>
> The current supported mode for enabling GPU workloads in the TEE scenario
> is single GPU passthrough (one GPU per pod) on AMD64 platforms (AMD SEV-SNP
> being the only supported TEE scenario so far with support for Intel TDX being
> on the way).

## Component Overview

Before providing deployment guidance, we describe the components involved to
support running GPU workloads. We start from a top to bottom perspective
from the NVIDIA GPU operator via the Kata runtime to the components within
the NVIDIA GPU Utility Virtual Machine (UVM) root filesystem.

### NVIDIA GPU Operator

A central component is the
[NVIDIA GPU operator](https://github.com/NVIDIA/gpu-operator) which can be
deployed onto your cluster as a helm chart. Installing the GPU operator
delivers various operands on your nodes in the form of Kubernetes DaemonSets.
These operands are vital to support the flow of orchestrating pod manifests
using NVIDIA GPU runtime classes with GPU passthrough on your nodes. Without
getting into the details, the most important operands and their
responsibilities are:

- **nvidia-vfio-manager:** Binding discovered NVIDIA GPUs to the `vfio-pci`
  driver for VFIO passthrough.
- **nvidia-cc-manager:** Transitioning GPUs into confidential computing (CC)
  and non-CC mode (see the
  [NVIDIA/k8s-cc-manager](https://github.com/NVIDIA/k8s-cc-manager)
  repository).
- **nvidia-kata-manager:** Creating host-side CDI specifications for GPU
  passthrough, resulting in the file `/var/run/cdi/nvidia.yaml`, containing
  `kind: nvidia.com/pgpu` (see the
  [NVIDIA/k8s-kata-manager](https://github.com/NVIDIA/k8s-kata-manager)
  repository).
- **nvidia-sandbox-device-plugin** (see the
  [NVIDIA/sandbox-device-plugin](https://github.com/NVIDIA/sandbox-device-plugin)
  repository):
  - Allocating GPUs during pod deployment.
  - Discovering NVIDIA GPUs, their capabilities, and advertising these to
    the Kubernetes control plane (allocatable resources as type
    `nvidia.com/pgpu` resources will appear for the node and GPU Device IDs
    will be registered with Kubelet). These GPUs can thus be allocated as
    container resources in your pod manifests. See below GPU operator
    deployment instructions for the use of the key `pgpu`, controlled via a
    variable.

To summarize, the GPU operator manages the GPUs on each node, allowing for
simple orchestration of pod manifests using Kata Containers. Once the cluster
with GPU operator and Kata bits is up and running, the end user can schedule
Kata NVIDIA GPU workloads, using resource limits and the
`kata-qemu-nvidia-gpu` or `kata-qemu-nvidia-gpu-snp` runtime classes, for
example:

```yaml
apiVersion: v1
kind: Pod
...
spec:
  ...
  runtimeClassName: kata-qemu-nvidia-gpu-snp
  ...
    resources:
      limits:
        "nvidia.com/pgpu": 1
...
```

When this happens, the Kubelet calls into the sandbox device plugin to
allocate a GPU. The sandbox device plugin returns `DeviceSpec` entries to the
Kubelet for the allocated GPU. The Kubelet uses internal device IDs for
tracking of allocated GPUs and includes the device specifications in the CRI
request when scheduling the pod through containerd. Containerd processes the
device specifications and includes the device configuration in the OCI
runtime spec used to invoke the Kata runtime during the create container
request.

### Kata runtime

The Kata runtime for the NVIDIA GPU handlers is configured to cold-plug VFIO
devices (`cold_plug_vfio` is set to `root-port` while
`hot_plug_vfio` is set to `no-port`). Cold-plug is by design the only
supported mode for NVIDIA GPU passthrough of the NVIDIA reference stack.

With cold-plug, the Kata runtime attaches the GPU at VM launch time, when
creating the pod sandbox. This happens *before* the create container request,
i.e., before the Kata runtime receives the OCI spec including device
configurations from containerd. Thus, a mechanism to acquire the device
information is required. This is done by the runtime calling the
`coldPlugDevices()` function during sandbox creation. In this function,
the runtime queries Kubelet's Pod Resources API to discover allocated GPU
device IDs (e.g., `nvidia.com/pgpu = [vfio0]`). The runtime formats these as
CDI device identifiers and injects them into the OCI spec using
`config.InjectCDIDevices()`. The runtime then consults the host CDI
specifications and determines the device path the GPU is backed by
(e.g., `/dev/vfio/devices/vfio0`). Finally, the runtime resolves the device's
PCI BDF (e.g., `0000:21:00`) and cold-plugs the GPU by launching QEMU with
relevant parameters for device passthrough (e.g.,
`-device vfio-pci,host=0000:21:00.0,x-pci-vendor-id=0x10de,x-pci-device-id=0x2321,bus=rp0,iommufd=iommufdvfio-faf829f2ea7aec330`).

The runtime also creates *inner runtime* CDI annotations
which map host VFIO devices to guest GPU devices. These are annotations
intended for the kata-agent, here referred to as the inner runtime (inside the
UVM), to properly handle GPU passthrough into containers. These annotations
serve as metadata providing the kata-agent with the information needed to
attach the passthrough devices to the correct container.
The annotations are key-value pairs consisting of `cdi.k8s.io/vfio<num>` keys
(derived from the host VFIO device path, e.g., `/dev/vfio/devices/vfio1`) and
`nvidia.com/gpu=<index>` values (referencing the corresponding device in the
guest CDI spec). These annotations are injected by the runtime during container
creation via the `annotateContainerWithVFIOMetadata` function (see
`container.go`).

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
trustee's Key Broker Service (KBS) for secure release of the NGC API
authentication key used to access the NVCR container registry. As part of
this, the attestation agent exercises composite attestation and transitions
the GPU into `Ready` state (without this, the GPU has to explicitly be
transitioned into `Ready` state by passing the `nvrc.smi.srs=1` kernel
parameter via the shim config, causing NVRC to transition the GPU into the
`Ready` state).

## Deployment Guidance

This guidance assumes you use bare-metal machines with proper support for
Kata's non-TEE and TEE GPU workload deployment scenarios for your Kubernetes
nodes. We provide guidance based on the upstream Kata CI procedures for the
NVIDIA GPU CI validation jobs. Note that, this setup:

- uses the guest image pull method to pull container image layers
- uses the genpolicy tool to attach Kata agent security policies to the pod
  manifest
- has dedicated (composite) attestation tests, a CUDA vectorAdd test, and a
  NIM/RA test sample with secure API key release

A similar deployment guide and scenario description can be found in NVIDIA resources
under
[Early Access: NVIDIA GPU Operator with Confidential Containers based on Kata](https://docs.nvidia.com/datacenter/cloud-native/gpu-operator/latest/confidential-containers.html).

### Requirements

The requirements for the TEE scenario are:

- Ubuntu 25.10 as host OS
- CPU with AMD SEV-SNP support with proper BIOS/UEFI version and settings
- CC-capable Hopper/Blackwell GPU with proper VBIOS version.

BIOS and VBIOS configuration is out of scope for this guide. Other resources,
such as the documentation found on the
[NVIDIA Trusted Computing Solutions](https://docs.nvidia.com/nvtrust/index.html)
page and the above linked NVIDIA documentation, provide guidance on
selecting proper hardware and on properly configuring its firmware and OS.

### Installation

#### Containerd and Kubernetes

First, set up your Kubernetes cluster. For instance, in Kata CI, our NVIDIA
jobs use a single-node vanilla Kubernetes cluster with a 2.x containerd
version and Kata's current supported Kubernetes version. We set this cluster
up using the `deploy_k8s` function from `tests/integration/kubernetes/gha-run.sh`
as follows:

```bash
$ export KUBERNETES="vanilla"
$ export CONTAINER_ENGINE="containerd"
$ export CONTAINER_ENGINE_VERSION="v2.1"
$ source tests/gha-run-k8s-common.sh
$ deploy_k8s
```

> **Note:**
>
> We recommend to configure your Kubelet with a higher
> `runtimeRequestTimeout` timeout value than the two minute default timeout.
> Using the guest-pull mechanism, pulling large images may take a significant
> amount of time and may delay container start, possibly leading your Kubelet
> to de-allocate your pod before it transitions from the *container created*
> to the *container running* state.

> **Note:**
>
> The NVIDIA GPU runtime classes use VFIO cold-plug which, as
> described above, requires the Kata runtime to query Kubelet's Pod Resources
> API to discover allocated GPU devices during sandbox creation. For
> Kubernetes versions **older than 1.34**, you must explicitly enable the
> `KubeletPodResourcesGet` feature gate in your Kubelet configuration. For
> Kubernetes 1.34 and later, this feature is enabled by default.

#### GPU Operator

Assuming you have the helm tools installed, deploy the latest version of the
GPU Operator as a helm chart (minimum version: `v25.10.0`):

```bash
$ helm repo add nvidia https://helm.ngc.nvidia.com/nvidia && helm repo update
$ helm install --wait --generate-name \
    -n gpu-operator --create-namespace \
    nvidia/gpu-operator \
    --set sandboxWorkloads.enabled=true \
    --set sandboxWorkloads.defaultWorkload=vm-passthrough \
    --set kataManager.enabled=true \
    --set kataManager.config.runtimeClasses=null \
    --set kataManager.repository=nvcr.io/nvidia/cloud-native \
    --set kataManager.image=k8s-kata-manager \
    --set kataManager.version=v0.2.4 \
    --set ccManager.enabled=true \
    --set ccManager.defaultMode=on \
    --set ccManager.repository=nvcr.io/nvidia/cloud-native \
    --set ccManager.image=k8s-cc-manager \
    --set ccManager.version=v0.2.0 \
    --set sandboxDevicePlugin.repository=nvcr.io/nvidia/cloud-native \
    --set sandboxDevicePlugin.image=nvidia-sandbox-device-plugin \
    --set sandboxDevicePlugin.version=v0.0.1 \
    --set 'sandboxDevicePlugin.env[0].name=P_GPU_ALIAS' \
    --set 'sandboxDevicePlugin.env[0].value=pgpu' \
    --set nfd.enabled=true \
    --set nfd.nodefeaturerules=true
```

> **Note:**
>
> For heterogeneous clusters with different GPU types, you can omit
> the `P_GPU_ALIAS` environment variable lines. This will cause the sandbox
> device plugin to create GPU model-specific resource types (e.g.,
> `nvidia.com/GH100_H100L_94GB`) instead of the generic `nvidia.com/pgpu`,
> which in turn can be used by pods through respective resource limits.
> For simplicity, this guide uses the generic alias.

> **Note:**
>
> Using `--set sandboxWorkloads.defaultWorkload=vm-passthrough` causes all
> your nodes to be labeled for GPU VM passthrough. Remove this parameter if
> you intend to only use selected nodes for this scenario, and label these
> nodes by hand, using:
> `kubectl label node <node-name> nvidia.com/gpu.workload.config=vm-passthrough`.

#### Kata Containers

Install the latest Kata Containers helm chart, similar to
[existing documentation](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/README.md)
(minimum version: `3.24.0`).

```bash
$ export VERSION=$(curl -sSL https://api.github.com/repos/kata-containers/kata-containers/releases/latest | jq .tag_name | tr -d '"')
$ export CHART="oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy"

$ helm install kata-deploy \
    --namespace kata-system \
    --create-namespace \
    -f "https://raw.githubusercontent.com/kata-containers/kata-containers/refs/tags/${VERSION}/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-nvidia-gpu.values.yaml" \
    --set nfd.enabled=false \
    --set shims.qemu-nvidia-gpu-tdx.enabled=false \
    --wait --timeout 10m --atomic \
    "${CHART}" --version "${VERSION}"
```

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

If you did not use the `sandboxWorkloads.defaultWorkload=vm-passthrough`
parameter during GPU operator deployment, label your nodes for GPU VM
passthrough, for the example of using all nodes for GPU passthrough, run:

```bash
$ kubectl label nodes --all nvidia.com/gpu.workload.config=vm-passthrough --overwrite
```

Check if the `nvidia-cc-manager` pod is running if you intend to run GPU TEE
scenarios. If not, you need to manually label the node as CC capable. Current
GPU Operator node feature rules do not yet recognize all CC capable GPU PCI
IDs. Run the following command:

```bash
$ kubectl label nodes --all nvidia.com/cc.capable=true
```

After this, assure the `nvidia-cc-manager` pod is running. With the suggested
parameters for GPU Operator deployment, the `nvidia-cc-manager` will
automatically transition the GPU into CC mode.

After deployment, you can transition your node(s) to the desired CC state,
using either the `on` or `off` value, depending on your scenario. For the
non-CC scenario, transition to the `off` state via:
`kubectl label nodes --all nvidia.com/cc.mode=off` and wait until all pods
are back running. When an actual change is exercised, various GPU operator
operands will be restarted.

Ensure all pods are running:

```bash
$ kubectl get pods -A
```

On your node(s), ensure for correct driver binding. Your GPU device should be
bound to the VFIO driver, i.e., showing `Kernel driver in use: vfio-pci`
when running:

```bash
$ lspci -nnk -d 10de:
```

### Run the CUDA vectorAdd sample

Create the following file:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: cuda-vectoradd-kata
  namespace: default
  annotations:
    io.katacontainers.config.hypervisor.kernel_params: "nvrc.smi.srs=1"
spec:
  runtimeClassName: ${GPU_RUNTIME_CLASS_NAME}
  restartPolicy: Never
  containers:
  - name: cuda-vectoradd
    image: "nvcr.io/nvidia/k8s/cuda-sample:vectoradd-cuda12.5.0-ubuntu22.04"
    resources:
      limits:
        nvidia.com/pgpu: "1"
        memory: 16Gi
```

Depending on your scenario and on the CC state, export your desired runtime
class name define the environment variable:

```bash
$ export GPU_RUNTIME_CLASS_NAME="kata-qemu-nvidia-gpu-snp"
```

Then, deploy the sample Kubernetes pod manifest and observe the pod logs:

```bash
$ envsubst < ./cuda-vectoradd-kata.yaml.in | kubectl apply -f -
$ kubectl wait --for=condition=Ready pod/cuda-vectoradd-kata --timeout=60s
$ kubectl logs -n default cuda-vectoradd-kata
```

Expect the following output:

```
[Vector addition of 50000 elements]
Copy input data from the host memory to the CUDA device
CUDA kernel launch with 196 blocks of 256 threads
Copy output data from the CUDA device to the host memory
Test PASSED
Done
```

To stop the pod, run: `kubectl delete pod cuda-vectoradd-kata`.

### Next steps

#### Transition between CC and non-CC mode

Use the previously described node labeling approach to transition between
the CC and non-CC mode. In case of the non-CC mode, you can use the
`kata-qemu-nvidia-gpu` value for the `GPU_RUNTIME_CLASS_NAME` runtime class
variable in the above CUDA vectorAdd sample. The `kata-qemu-nvidia-gpu-snp`
runtime class will **NOT** work in this mode - and vice versa.

#### Run Kata CI tests locally

Upstream Kata CI runs the CUDA vectorAdd test, a composite attestation test,
and a basic NIM/RAG deployment. Running CI tests for the TEE GPU scenario
requires KBS to be deployed (except for the CUDA vectorAdd test). The best
place to get started running these tests locally is to look into our
[NVIDIA CI workflow manifest](https://github.com/kata-containers/kata-containers/blob/main/.github/workflows/run-k8s-tests-on-nvidia-gpu.yaml)
and into the underling
[run_kubernetes_nv_tests.sh](https://github.com/kata-containers/kata-containers/blob/main/tests/integration/kubernetes/run_kubernetes_nv_tests.sh)
script. For example, to run the CUDA vectorAdd scenario against the TEE GPU
runtime class use the following commands:

```bash
# create the kata runtime class the test framework uses
$ export KATA_HYPERVISOR=qemu-nvidia-gpu-snp
$ kubectl delete runtimeclass kata --ignore-not-found
$ kubectl get runtimeclass "kata-${KATA_HYPERVISOR}" -o json | \
    jq '.metadata.name = "kata" | del(.metadata.uid, .metadata.resourceVersion, .metadata.creationTimestamp)' | \
    kubectl apply -f -
$ cd tests/integration/kubernetes
$ K8S_TEST_NV="k8s-nvidia-cuda.bats" ./gha-run.sh run-nv-tests
```

> **Note:**
>
> The other scenarios require an NGC API key to run, i.e., to export the
> `NGC_API_KEY` variable with a valid NGC API key.

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

#### Deploy pods with Kata agent security policies

With GPU passthrough being supported by the
[genpolicy tool](https://github.com/kata-containers/kata-containers/tree/main/src/tools/genpolicy),
you can use the tool to create a Kata agent security policy. Our CI deploys
all sample pod manifests with a Kata agent security policy.

#### Deploy pods using your own containers and manifests

You can author pod manifests leveraging your own containers, for instance,
containers built using the CUDA container toolkit. We recommend to start
with a CUDA base container.

The GPU is transitioned into the `Ready` state via attestation, for instance,
when pulling authenticated images. If your deployment scenario does not use
attestation, please refer back to the CUDA vectorAdd pod manifest. In this
manifest, we ensure that NVRC sets the GPU to `Ready` state by adding the
following annotation in the manifest:
`io.katacontainers.config.hypervisor.kernel_params: "nvrc.smi.srs=1"`

> **Notes:**
>
> - musl-based container images (e.g., using Alpine), or distro-less
>   containers are not supported.
> - for the TEE scenario, only single-GPU passthrough per pod is supported,
>   so your pod resource limit must be: `nvidia.com/pgpu: "1"` (on a system
>   with multiple GPUs, you can thus pass through one GPU per pod).

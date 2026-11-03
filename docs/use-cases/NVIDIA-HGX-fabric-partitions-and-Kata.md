# NVIDIA HGX fabric partitions and Kata

On an HGX system the GPUs are connected through NVSwitches. In *shared NVSwitch mode* Fabric Manager (FM) runs in a Service VM that holds only the switch fabric, and it activates *partitions*: sets of GPUs that may reach each other over NVLink. A tenant VM must receive exactly one active partition's GPUs. Two tenants inside one partition would share an NVLink domain, so the partition is the isolation boundary. The switches' PCI face differs by generation: on H100 and H200 the NVSwitches are PCIe devices of their own; from B200 on they are reached through ConnectX management functions.

This page describes the components that make a Kata cluster do this, the configurations they can run in, the day-to-day workflows, and how to read what the system says when something is off. Everything here was run on a DGX with eight GPUs and four ConnectX management PFs, Kubernetes 1.37, Kata 4.3.1 and Fabric Manager 615.

*[FM]: Fabric Manager
*[PF]: PCI physical function
*[PFs]: PCI physical functions
*[CDI]: Container Device Interface
*[CID]: vsock context identifier

## The model

### Who owns what

Four components, four repositories, one contract between them.

kata-device-provisioner
:   Runs once per node as a Job on the trusted side. Binds the GPUs and the fabric's management functions to `vfio-pci`, writes the boot persistence for that binding, and publishes a node fact for the fabric. It changes host state; nothing else does.

kata-device-plugin
:   A DaemonSet that enumerates the node's VFIO devices and declares them to the kubelet. Read-only toward the host: it binds nothing. It also hosts the *fabric view* service that the Service VM publishes to. No Kubernetes API access.

NVRC
:   The init of the Kata guest image for NVIDIA systems. It detects what the VM received, starts Fabric Manager and NVLSM in a Service VM or the GPU driver and persistenced in a tenant VM, waits for each service to be ready, and only then starts the kata-agent.

fmclient
:   A client for Fabric Manager's SDK. It lists, activates and deactivates partitions, and every change publishes the partition view to the plugin in one call. It runs inside the Service VM or on the host, the two configurations defined under [The view channel](#the-view-channel). It also owns the Service VM manifests.

The shared vocabulary lives in `pcilibs-rs`: which PCI identities are GPUs, NVSwitches or management functions, the partition view's wire format, the contract constants (environment variable names, socket path, port, the Service VM's CID), and the resolution from Fabric Manager's physical IDs to host PCI addresses.

### The fabric is one device

The plugin advertises the node's whole NVSwitch fabric as one extended resource, `nvidia.com/nvswitch-fabric`, with capacity 1 or 0. On H100 systems that is the four NVSwitch chips; on B200, B300 and Rubin systems it is the ConnectX management PFs through which NVLSM and FM manage the switches. The fabric is advertised only when every function of it is bound to `vfio-pci`; a half-bound fabric is withheld, because a Service VM with half a fabric cannot manage anything.

Its only consumer is the Service VM, which requests `nvidia.com/nvswitch-fabric: 1`. The plugin's `Allocate` for this device does one extra thing: it mints a fresh token and hands the container two environment variables, `KATA_FABRIC_VIEW_ENDPOINT` and `KATA_FABRIC_VIEW_TOKEN`. Only the fabric's holder learns where to publish and with which token.

### Partitions are resources

Fabric Manager knows the eight GPUs of this box as fifteen partitions: one of all eight, two halves, four pairs and eight singles. When an operator activates a partition, the plugin advertises it as its own extended resource, named by size:

| Active partition | Resource |
| --- | --- |
| 4 GPUs | `nvidia.com/nvlink-4gpu: 1` |
| 2 GPUs | `nvidia.com/nvlink-2gpu: 1` |
| 1 GPU | `nvidia.com/gpu: 1` |

Each unit is one device whose CDI entry carries every GPU function of the partition, in Fabric Manager's member order. A pod requesting `nvidia.com/nvlink-4gpu: 1` gets those four GPUs and no others. `nvidia.com/gpu: N` was rejected for partitions: the count cannot say whether four GPUs are NVLinked, standalone or PCIe, and only a per-shape resource lets the scheduler see how many partitions of a shape remain.

An active partition is advertised Healthy when it is supported by Fabric Manager, exclusive (no other active partition claims one of its GPUs) and complete (every member GPU is bound to `vfio-pci` and carries a readable slot label). Otherwise it is advertised Unhealthy under the same name: counted in capacity, left out of allocatable, and the plugin logs the reason. An active partition with no member bound on this host is not advertised.

### The view channel

Fabric Manager inside the Service VM is the only authority on partitions, and it knows GPUs by *physical ID*, the module slot on the baseboard. The host knows GPUs by PCI address. Two facts bridge them:

- fmclient publishes Fabric Manager's partition lists, the *view*, to the plugin in one gRPC call, and the plugin keeps that view, on disk next to its token so a plugin restart keeps the node's capacity, until the next call or until the fabric changes hands. Every fmclient command that changes partitions publishes the result; nothing stays connected. The plugin is a scheduling ledger and Fabric Manager is the enforcement point: a stale view sends a pod to GPUs whose fabric is not ready, where CUDA init fails, and cannot hand out another partition's GPUs.
- The firmware labels every GPU function in sysfs: `/sys/bus/pci/devices/<bdf>/label` reads `GPU4, SMX3` on this DGX. The `SMX` number is Fabric Manager's physical ID; the `GPU` number is only PCI enumeration order.

The plugin listens twice: on a TCP port, 9877, exposed as a hostPort for the in-band publisher inside the Service VM, admitting only the token minted at `Allocate`; and on a unix socket, `/var/run/kata-device-plugin/fabric-view.sock`, for a publisher on the host itself, where filesystem permissions are the credential and no token is checked. A new fabric holder withdraws the view: its `Allocate` rotates the token, and the previous holder's token no longer publishes.

Where fmclient runs names the configuration. *In-band*: inside the Service VM's container, through `kubectl exec`, reaching Fabric Manager on the guest's loopback and the plugin on the token-gated port. *Out-of-band*: on the host, reaching Fabric Manager over vsock and the plugin on the unix socket. A node runs one of the two.

```mermaid
flowchart LR
    subgraph svm["Service VM (holds the fabric)"]
        FM[Fabric Manager] --- NVLSM
        fmc[fmclient -a N] -->|SDK, loopback| FM
    end
    subgraph host["Node"]
        plugin[kata-device-plugin]
        kubelet
        prov[kata-device-provisioner]
    end
    fmc -->|view over gRPC, token| plugin
    plugin -->|nvidia.com/nvlink-4gpu, nvswitch-fabric| kubelet
    prov -->|binds to vfio-pci| host
    tenant["Tenant VM: 4 GPUs of partition 2"] -->|NVLink inband probe| FM
```

### Trust boundaries

Provider-owned
:   The infrastructure owns the hardware and the Service VM, in Kata and in Confidential Containers alike. The host may hold a control path into the Service VM. Either the in-band or the out-of-band configuration is admissible, one per node: the two Service VM manifests differ in runtime class and annotations, and a node runs one.

Colocation
:   The tenant owns the hardware and the Service VM and carves partitions themselves, outside the infrastructure's reach. The host must have no Fabric Manager control path. Fabric Manager's API has no read-only mode, so there is no vsock listener at all; the in-band configuration is the only one. Fabric Manager's vsock listener is therefore off unless the Service VM's own kernel command line turns it on.

The plugin never reads anything from the Service VM's filesystem or from the shim's private state: the view arrives as a protocol message or not at all.

!!! warning "Only the fabric owner may request the fabric"
    Whoever holds `nvidia.com/nvswitch-fabric` decides which GPUs share NVLink and publishes the view the node advertises, so a tenant holding it has a data path into other tenants' GPUs. The plugin cannot refuse a requester: the kubelet's Allocate carries device IDs and no pod identity. The kata-device-plugin chart ships a `ValidatingAdmissionPolicy` that denies the request outside the namespaces in its `fabricOwnerNamespaces` value (Kubernetes 1.30 or later). The runtime class whose `vsock_cid` and `kernel_params` annotations the Service VM relies on needs the same gate, since a tenant pod at CID 3 would answer the host publisher in Fabric Manager's place, and tenants hold no `pods/exec` in the owner namespace. In the colocation model the owner namespace is the tenant's own; the gate is the same.

## Configurations

A node runs in one of three deployment modes:

1. **GPU only, no fabric.** No Fabric Manager anywhere. The switches are never programmed and each GPU works alone over PCIe.
2. **GPU with the host's Fabric Manager in `FABRIC_MODE=0`.** Fabric Manager and NVLSM run on the host and program the default partition, all eight GPUs in one NVLink domain. VMs of any size share that domain; in CC mode each VM that holds more than one GPU derives its own NVLink encryption key.
3. **Service VM.** Fabric Manager runs in a Service VM in shared NVSwitch mode and activates partitions, which the node advertises as resources. The variants below, the `gpu` unit, out-of-band over vsock and the fail-closed state, all belong to this mode.

Each configuration below lists the exact commands and what the node advertises afterwards. What the node advertises follows from two facts: whether the fabric is bound to `vfio-pci`, and whether a view was published. Modes 1 and 2 share the same two facts, fabric not bound and no view, so the plugin treats them alike.

| Fabric bound | View | Unit in view | Node advertises |
| --- | --- | --- | --- |
| no | none | n/a | `nvidia.com/gpu: N`, one per bound GPU (modes 1 and 2) |
| yes | none | n/a | `nvidia.com/nvswitch-fabric: 1`, no GPUs |
| yes | published | `partition` | the fabric plus one device per active partition, Unhealthy unless Fabric Manager supports it, no other active partition shares a GPU with it, and every member is bound and labeled here |
| yes | published | `gpu` | the fabric plus `nvidia.com/gpu: N`, one per bound GPU, whatever the partition list says |

A published view decides what the node advertises; the fabric's binding only tells the plugin whether to wait for one.

### GPU mode, no fabric

The GPUs are on `vfio-pci`, the fabric functions stay on the host's driver, and nothing programs the switches: neither Fabric Manager nor NVLSM is installed on the host. This is also what a node without NVSwitches looks like.

```bash title="$ provision the GPUs only"
kata-device-provisioner apply --mode off
```

```bash title="$ what the kubelet shows"
kubectl get node <node> -o jsonpath='{.status.capacity}' | tr ',' '\n' | grep nvidia
"nvidia.com/gpu":"8"
"nvidia.com/nvswitch-fabric":"0"
```

A pod requests `nvidia.com/gpu: 1` with `runtimeClassName: kata-qemu-nvidia-gpu` and gets one GPU. Any size up to the node's count is allocatable. Each GPU works on its own over PCIe; with no Fabric Manager anywhere the switch routing is never programmed, so the GPUs of one pod have no NVLink between them.

### GPU mode, host Fabric Manager in `FABRIC_MODE=0`

Fabric Manager and NVLSM run on the host with `FABRIC_MODE=0` in `/usr/share/nvidia/nvswitch/fabricmanager.cfg` and program the default partition: all eight GPUs in one NVLink domain. The management PFs stay on `mlx5_core` for them; only the GPUs go to `vfio-pci`. The plugin sees the same two facts as in mode 1 and advertises the same.

```bash title="$ on the host, NVLSM before Fabric Manager"
systemctl enable --now nvlsm
systemctl enable --now nvidia-fabricmanager
kata-device-provisioner apply --mode off     # --mode on for a CC node
```

```bash title="$ what the kubelet shows"
"nvidia.com/gpu":"8"
"nvidia.com/nvswitch-fabric":"0"
```

A pod requests `nvidia.com/gpu: N` for any N up to eight, and its GPUs reach each other over NVLink because every GPU sits in the one partition. All VMs on the node share that NVLink domain. In CC mode each VM that holds more than one GPU derives its own NVLink encryption key, so its traffic is unreadable to the GPUs of another VM. Shared NVSwitch mode, next, is the configuration in which the fabric itself separates tenants.

### Service VM with the view, partition unit

The fabric is bound, the Service VM holds it, and the view's unit is `partition`, fmclient's default. This is the shared NVSwitch mode the HGX guide describes. How the provisioner itself is deployed, as a Job through its chart and profiles, is in the [kata-device-provisioner](https://github.com/kata-containers/kata-device-provisioner) README; this page shows only the call.

```bash title="$ bind the fabric"
kata-device-provisioner status                      # every target function and its current driver
echo 0000:27:00.0 > /sys/bus/pci/devices/0000:27:00.0/driver/unbind   # once per management PF on mlx5_core
kata-device-provisioner apply --mode off --bind-fabric
```

The provisioner refuses a function another driver holds, so each management PF is unbound by address first; `mlx5_core` stays loaded for the node's other NICs. The host runs no fabric service in this model. A node whose OS image ships Fabric Manager and NVLSM, as a DGX OS does, disables them before the unbind, since they use those PFs.

```yaml title="deploy/service-vm.yaml (fmclient repository), the parts that matter"
runtimeClassName: kata-qemu-nvidia-gpu
nodeSelector:
  kata.feature.node.kubernetes.io/managed-fabric: "true"
containers:
  - name: fmclient
    command: ["sleep", "infinity"]
    resources:
      limits:
        nvidia.com/nvswitch-fabric: 1
```

Before the Service VM publishes, the node advertises the fabric and no GPUs:

```text
"nvidia.com/gpu":"0"
"nvidia.com/nvswitch-fabric":"1"
```

After `fmclient -a 2`, `-a 6` and `-a 15` inside the Service VM, a half, a pair and a single:

```text
"nvidia.com/gpu":"1"
"nvidia.com/nvlink-2gpu":"1"
"nvidia.com/nvlink-4gpu":"1"
"nvidia.com/nvswitch-fabric":"1"
```

### Service VM with the view, GPU unit

Same deployment; the view is published with `--unit=gpu`. The plugin then advertises every bound GPU as its own `nvidia.com/gpu`, whatever the partition list says, and a pod may take any number of them; the plugin does not check which partition a pod's GPUs come from. This is bare-metal semantics for an owner who holds the whole node as one trust domain, with one partition of all GPUs active. It is wrong for a node shared by mutually untrusted tenants: two pods could each receive GPUs of the same active partition and share its NVLink domain, the boundary this page starts with. Such a node uses the partition unit, where a pod gets a whole partition or nothing.

```bash title="$ the only change"
kubectl -n kata-system exec $P -- fmclient --publish --unit=gpu
```

```text
"nvidia.com/gpu":"8"
"nvidia.com/nvswitch-fabric":"1"
```

### Out-of-band over vsock

The host activates partitions itself over vsock, without `kubectl exec`, and each change publishes the view to the plugin's host socket.

!!! info "Prerequisites in review"
    This recipe needs kata runtime-rs with the configured `vsock_cid` ([kata-containers#14202](https://github.com/kata-containers/kata-containers/pull/14202)), NVRC with `nvrc.fm.vsock` ([nvrc#244](https://github.com/NVIDIA/nvrc/pull/244)) and with the CDI spec for ConnectX functions that runtime-rs's `nvidia.com/nic` annotations resolve against ([nvrc#245](https://github.com/NVIDIA/nvrc/pull/245), a bridge until `nvidia-ctk cdi generate` gains a NIC mode, [nvidia-container-toolkit#2197](https://github.com/NVIDIA/nvidia-container-toolkit/issues/2197)), the vsock constants in pcilibs-rs ([pcilibs-rs#42](https://github.com/kata-containers/pcilibs-rs/pull/42)) and fmclient's one-call publish ([fmclient#9](https://github.com/zvonkok/fmclient/pull/9)). A runtime without the annotation handler refuses the `vsock_cid` annotation; the Go runtime has no `vsock_cid` setting, so it cannot run this configuration, while the in-band configuration works on both runtimes.

Three settings make it possible, all of them off by default:

- `io.katacontainers.config.hypervisor.vsock_cid: "3"` on the Service VM pod gives the VM a fixed CID. kata runtime-rs keeps 3 to 1023 for configured CIDs and draws random ones above that range; a configured CID already in use fails the sandbox.
- `io.katacontainers.config.hypervisor.kernel_params: "nvrc.fm.vsock=on"` makes NVRC write `FM_CMD_VSOCK_CID=VMADDR_CID_ANY` into Fabric Manager's configuration, so FM listens on vsock at its default port 1447971919.
- The runtime class must be the runtime-rs NVIDIA shim, `kata-qemu-nvidia-gpu-runtime-rs`, with both annotations in its `enable_annotations`.
- That shim runs without a shared filesystem and rejects an overlayfs container rootfs, so its pods need containerd's erofs snapshotter: `erofs-utils` 1.8.2 or newer on the host, and in the kata-deploy values `snapshotter.setup: ["erofs"]` plus `containerd.snapshotter: "erofs"` on the shim, as the chart's `try-kata-nvidia-gpu.values.yaml` profile sets them.

```yaml title="deploy/service-vm-out-of-band.yaml (fmclient repository), the parts that matter"
annotations:
  io.katacontainers.config.hypervisor.vsock_cid: "3"
  io.katacontainers.config.hypervisor.kernel_params: "nvrc.fm.vsock=on"
runtimeClassName: kata-qemu-nvidia-gpu-runtime-rs
containers:
  - name: fabric-holder
    command: ["sleep", "infinity"]
    resources:
      limits:
        nvidia.com/nvswitch-fabric: 1
```

```bash title="$ on the host, as root"
fmclient --vsock -l
fmclient --vsock -a 2     # publishes to the plugin's host socket
```

`--vsock` without a value is the contract address, CID 3 and FM's default port, both constants in `pcilibs-rs`; with it the plugin's host socket is fmclient's default endpoint. The node advertises the same resources as in the in-band configuration.

As run on the HGX B300: the shim connected to the agent at CID 3 and the pod was Running 50 s after `kubectl apply`, Fabric Manager having reached `configured` 11 s after the guest booted. `fmclient --vsock -l` on the host listed the 15 partitions, `fmclient --vsock -a 2` made the node advertise `nvidia.com/nvlink-4gpu: 1` within one plugin poll, and a tenant VM on that unit saw the partition's four GPUs with NVLink between all of them.

!!! warning "Any host process can connect"
    A vsock connect needs no privilege on the host, so with the listener on every host process can reach Fabric Manager's API. This is the provider-owned model. Where the host must have no control path, run the in-band manifest, whose guest opens nothing.

### Fail closed: fabric bound, no view

Between the provisioner binding the fabric and the first publish, and after a new Service VM pod took the fabric and has not published yet, the node advertises the fabric and no GPUs. The plugin logs

```text
WARN fabric is bound but no partition view is published: advertising no GPU
```

once per poll. This is the designed state: a GPU handed out without a view could land in a partition the operator did not activate. The way out is a publish from the fabric holder: `fmclient -a <id>`, or `fmclient --publish` to resend what Fabric Manager has.

## Workflows

### Provisioning the node

The provisioner binds every GPU and, with `--bind-fabric`, every function of the fabric; how it is deployed is in its own [README](https://github.com/kata-containers/kata-device-provisioner). It refuses before changing anything when a function is held by a host driver, when a target has no IOMMU group, or when a target shares its IOMMU group with a function a host driver still holds.

```bash title="$ as root on the node"
systemctl disable --now nvidia-fabricmanager nvlsm                     # (1)!
kata-device-provisioner status                                         # (2)!
echo 0000:27:00.0 > /sys/bus/pci/devices/0000:27:00.0/driver/unbind    # (3)!
kata-device-provisioner apply --mode off --bind-fabric
```

1. Only on a node whose OS image ships them: they hold the management PFs on `mlx5_core`, and the provisioner refuses to take a function from a host driver.
2. Lists what the node would provision and the driver each function has now.
3. Once per management PF `status` shows on `mlx5_core`, and per GPU a host driver holds: the provisioner refuses a function another driver holds, and unloading `mlx5_core` would take the node's other NICs down.

Persistence is written as `/etc/modprobe.d/kata-device-provisioner.conf` for the GPUs and address-scoped udev rules for the management functions. The node fact `kata.feature.node.kubernetes.io/managed-fabric=true` is published for Node Feature Discovery; without NFD, label the node by hand.

!!! bug "Host driver installed"
    With a host `nvidia` driver installed, the kernel autoloads it for the GPU's PCI identity and it binds the GPU before `vfio-pci` is loaded with its `options vfio-pci ids=`. On a node that still has the host driver package, the GPUs come back on `nvidia` after a reboot and have to be rebound. A Kata node without the driver package is unaffected.

### Bootstrapping the Service VM

Apply the manifest. The pod schedules once the plugin advertises the fabric. Inside the guest, NVRC detects `ServiceVm(ConnectX)` on these systems, starts NVLSM and Fabric Manager back to back, waits for Fabric Manager to report its fabric configured, and starts the agent. On the DGX that took 29.5 s from boot. The node advertises the fabric and no GPUs until the first `fmclient -a` publishes a view.

```bash title="$ watch it come up"
kubectl -n kata-system get pods -w
kubectl -n kube-system logs -l app.kubernetes.io/name=kata-device-plugin | grep fabric
```

```text
WARN fabric is bound but no partition view is published: advertising no GPU
```

### Activating and deactivating partitions

In-band, through the Service VM's container:

```bash title="$ the partition table, three activations, one deactivation"
P=$(kubectl -n kata-system get pod -l app=nvswitch-service-vm -o name)
kubectl -n kata-system exec $P -- fmclient -l
kubectl -n kata-system exec $P -- fmclient -a 2
kubectl -n kata-system exec $P -- fmclient -a 6
kubectl -n kata-system exec $P -- fmclient -a 15
kubectl -n kata-system exec $P -- fmclient -d 15    # -d deactivates
```

Each command publishes the view it leaves behind and prints it; within one plugin poll the node's capacity follows:

```text
fmclient: published to http://10.86.129.206:9877: unit "partition", active partitions (id x gpus): 2x4 6x2
```

Nothing here needs a person at `kubectl exec`. An owner that wants a fixed layout makes the container's command activate it before idling, `command: ["sh", "-c", "fmclient -a 2 && fmclient -a 6 && sleep infinity"]`, or runs the same calls from a Job in the owner namespace. A controller that turns node labels into these calls is possible on top; this page stops at the calls.

Deactivating a partition whose unit a pod holds does not take the GPUs away from that pod; the kubelet keeps its allocation. The resource stops being advertised for new pods.

### Scheduling a tenant VM

```yaml title="a tenant on the 4-GPU partition"
apiVersion: v1
kind: Pod
metadata:
  name: tenant-nvlink4
spec:
  runtimeClassName: kata-qemu-nvidia-gpu
  containers:
    - name: smi
      image: nvcr.io/nvidia/cuda:12.6.3-base-ubuntu24.04
      command: ["nvidia-smi", "-L"]
      resources:
        limits:
          nvidia.com/nvlink-4gpu: 1
```

The kubelet allocates the unit, the plugin answers with its CDI name, and the Kata shim cold-plugs the four VFIO devices listed in the CDI entry into the VM.

!!! note "DevicePluginCDIDevices"
    The kubelet forwards a device plugin's CDI names to the container runtime only with `DevicePluginCDIDevices`, which is on in every supported Kubernetes release. On a kubelet without it the VM still boots with the GPUs, since the shim learns them from the PodResources API, but the container inside sees no device.

### What a tenant sees

Inside the VM, NVRC detects `Gpu { fabric: None, gpus: 4 }`, loads the driver and starts persistenced, which registers the GPUs one by one. Each registration takes about 30 s on this box and reports the UVM step as "the NVLink fabric is not ready" before "persistence mode enabled". Fabric Manager in the Service VM logs the probe requests from those GPUs "on the partition 2" and brings their access links up.

```text title="nvidia-smi -L inside the tenant"
GPU 0: NVIDIA Graphics Device (UUID: GPU-dd58ef3d-...)
GPU 1: NVIDIA Graphics Device (UUID: GPU-4fc6688b-...)
GPU 2: NVIDIA Graphics Device (UUID: GPU-12f30f43-...)
GPU 3: NVIDIA Graphics Device (UUID: GPU-18a241c9-...)
```

`nvidia-smi topo -m` shows NVLink between all four and nothing else; Fabric Manager's table shows partition 2 active with its links training.

### Service VM restart and token rotation

The token is minted at `Allocate` and persisted on the host under `/var/run/kata-device-plugin`, so a plugin restart keeps the token a Service VM already holds. A container restart inside the same pod keeps the kubelet's allocation and the environment, so the same token keeps working and the view stays as it was.

A new Service VM pod is a new fabric holder. Its `Allocate` rotates the token: the previous holder's token no longer publishes, and the view is withdrawn at once, since the partitions it described were the previous holder's. Until the new holder publishes, the node is in the fail-closed state. Pods already running keep their GPUs: the kubelet keeps their allocation and the VMs their devices; only scheduling changes.

### Plugin restart

The plugin reloads the token and the last view from `/var/run/kata-device-plugin` and re-registers its resources, so the node's capacity comes back as it was. Pods already running keep their devices.

### Node reboot

The provisioner's persistence rebinds the fabric's management functions through its udev rules. The GPUs rebind through `vfio-pci`'s `ids=` option unless the host driver package is installed (see above). The host's `nvidia-fabricmanager.service`, if still enabled, fails its precheck because its PFs are now on `vfio-pci`; disable it. The Service VM DaemonSet recreates its pod and everything above repeats.

!!! bug "Stale Fabric Manager sessions on the GPUs"
    GPUs keep a session with the Fabric Manager that last configured them, and a VFIO reset does not clear it. After another Fabric Manager has run against the GPUs, the one a DGX OS ships on the host for example, the Service VM's Fabric Manager rejects the GPUs' inband responses with `previous session ID ... does not match the expected session ID` and lists no supported partitions. A node power cycle clears the sessions. Seen once, after the host's Fabric Manager had configured the GPUs before the Service VM's took over; a restart of the Service VM's own Fabric Manager was not tested, and FM's restart mode (`FABRIC_MODE_RESTART=1`, `fmclient --set-activated-list`) is the documented path for that.

## Readiness and diagnostics

### NVRC markers, per switch generation

NVRC starts a service and then blocks on the one syslog line that service writes when it is ready; every marker travels from the daemon's `syslog()` call through NVRC's shim on `/dev/log` into kmsg. The marker for Fabric Manager depends on the switch generation:

| Service | Platform | Marker |
| --- | --- | --- |
| Fabric Manager | H100 and H200, direct NVSwitches | `FM starting NvLink Inband` |
| Fabric Manager | B200, B300 and Rubin, ConnectX management PFs | ``fabric manager state is `3` (configured)`` |
| nvidia-persistenced | tenant and GPU-mode VMs | `Local RPC services initialized` |

NVLSM is not waited for on its own. On these systems the subnet manager becomes master only after Fabric Manager has connected to it and synced partitions, 70 ms after FM's request on this box, so a wait for OpenSM's `Entering MASTER state` before FM starts never ends. FM configured implies NVLSM master.

### The persistenced sequence

In a tenant VM, persistenced is the gate before the agent. The guest console shows the sequence per GPU:

```text
nvidia-persistenced: device 0000:03:00.0 - registered
nvidia-persistenced: device 0000:03:00.0 - Could not Enable UVM Persistence mode because the NVLink fabric is not ready: 0x81
nvidia-persistenced: device 0000:03:00.0 - persistence mode enabled.
nvidia-persistenced: device 0000:03:00.0 - NUMA memory onlined.
```

The NVLink fabric for a tenant GPU is ready only after Fabric Manager in the Service VM has accepted the GPU's inband probe on its partition; a tenant VM started while the Service VM is restarting waits here.

### Capturing the guest console

Two settings put the guest console into the node's journal: `enable_debug = true` in the configuration the shim loads, named by `ConfigPath` in the runtime's containerd entry: `/opt/kata/share/defaults/kata-containers/runtimes/qemu-nvidia-gpu/configuration-qemu-nvidia-gpu.toml` for the Go shim, `/opt/kata/share/defaults/kata-containers/runtime-rs/runtimes/qemu-nvidia-gpu-runtime-rs/configuration-qemu-nvidia-gpu-runtime-rs.toml` for runtime-rs and `[debug] level = "debug"` in `/etc/containerd/config.toml`, since the shim's debug lines are dropped at containerd's default level. The pod annotation `io.katacontainers.config.hypervisor.kernel_params: "nvrc.log=trace"` makes NVRC print every step.

```bash title="$ read the console of the last sandbox"
# the Go shim logs vmconsole="...", runtime-rs logs vm console: "..."
sudo journalctl --since "10 min ago" -o cat | grep -a -oE 'vmconsole="[^"]*"|vm console: "[^"]*"' | sed -E 's/^(vmconsole=|vm console: )"//; s/"$//'
```

!!! danger "Do not restart containerd with a Kata VM holding VFIO devices"
    On this kernel the VM killed by the restart left its devices wedged: new QEMUs failed to bind them to iommufd and an unbind blocked until the retrying QEMU released the device. Stop the pods first.

### Known failure signatures

| Where | Line | Meaning |
| --- | --- | --- |
| NVRC console | `NVRC panic: ... timeout waiting for: <marker>` | the service did not reach the marker; the lines before it say what it did instead |
| FM in the Service VM | `specified partition id N is not supported` | the partition list is empty or lacks N; see the stale-session case |
| kata-deploy install job | `DEFAULT_SHIM 'qemu-runtime-rs' must be one of the configured SHIMS` | the chart's `defaultShim` names a shim the values disabled |
| kata-deploy host check | `Required host command mkfs.erofs is not available` | the values ask for the erofs snapshotter on a node without `erofs-utils` |
| containerd | `PodResources API Get method disabled` | Kubernetes before 1.34 without the `KubeletPodResourcesGet` feature gate, which both runtimes call during cold plug; default on from 1.34 |
| guest kernel | `cannot find valid erofs superblock` | an image built as ext4 for a runtime configured with `rootfs_type = "erofs"` |
| guest NVRC | `Module nvidia not found in directory /lib/modules/<version>` | an image whose kernel modules do not match the installed guest kernel |
| containerd | `unsupported rootfs Mount { source: "overlay", ... }` | a runtime-rs NVIDIA pod on the overlayfs snapshotter; the shim has no shared filesystem and needs the erofs snapshotter |
| Go shim, after a kubelet restart | `No devices found in Pod Resources, skip cold plug` | the pod was scheduled on the capacity the kubelet kept across its restart, before the plugin re-registered the resource, so the kubelet allocated nothing and the VM booted without GPUs, NVRC in `Cpu` mode. Wait for the plugin's `device list changed` after a kubelet restart, or recreate the pod |
| kubelet | `nvidia.com/nvlink-<k>gpu` in capacity but not in allocatable | an active partition the plugin advertises Unhealthy: unsupported by Fabric Manager, overlapping another active partition, or a member GPU not bound and labeled on this host; the plugin's log names the partition and the reason |
| kata-agent | `waiting for CDI spec(s) to be generated (N of 100 max tries) "unresolvable CDI devices nvidia.com/nic=0, ..."` | runtime-rs annotates every ConnectX function as `nvidia.com/nic=<index>` and the guest wrote no spec for that kind; the container fails after 100 s while the VM and Fabric Manager keep running. An NVRC without the ConnectX spec generator |
| NVLSM in the Service VM | `GRPC_MGR - ... Failed to open configuration file "/usr/share/nvidia/nvlsm/grpc_mgr.conf"` | the base plus extension guest image with an NVRC that does not bind the extension's `/usr/share/nvidia` over the base path; Fabric Manager then exits with `failed to connect to the subnet manager` |

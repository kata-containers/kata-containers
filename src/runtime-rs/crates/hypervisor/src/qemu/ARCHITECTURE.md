# QEMU Command-Line Architecture

> **Status:** Migration in progress — see the [phase tracker](#migration-phases) below.
> **Issues:** [#12187](https://github.com/kata-containers/kata-containers/issues/12187) (refactor),
>             [#12125](https://github.com/kata-containers/kata-containers/issues/12125) (NUMA / hugepages)

> **Living document:** This file is updated with every PR that lands a migration
> phase.  It intentionally contains open questions and decision notes while the
> migration is in progress.  Once Phase 6 completes, all such notes will be
> removed and the document will serve as the definitive, stable reference for the
> QEMU command-line architecture — no TODOs, no decision stubs, just the design
> as implemented.

## Overview

This document describes the target machine-centric architecture for the QEMU
command-line generator in `runtime-rs`, why the current design needs changing,
and how each migration phase moves the code toward the target.

The [Grace Platform Configurations](#grace-platform-configurations) section
enumerates 7 concrete command-line topologies derived from tested production
deployments.  Each becomes a golden test fixture; the architecture must be
able to generate every one of them exactly from typed `Platform` inputs.

---

## Current State (the Problem)

All QEMU command-line construction lives in `cmdline_generator.rs` (~3 700 lines).
The design is inspired by `govmm` and works well for simple cases, but has
accumulated three structural problems:

### 1. Architecture decisions scattered across devices

Whether a virtio device needs `bus=pcie.0` depends on the machine layout
(Q35 with `pxb-pcie`, `virt` with extra root ports, NUMA enabled, …), yet every
device struct encodes that decision itself via `#[cfg(target_arch = ...)]`:

```rust
// Only valid for Q35 or VIRT machine types aka x86 or aarch64
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
params.push("bus=pcie.0".to_owned());
```

This pattern is duplicated across `DeviceVhostUserFs`, `DeviceVirtioBlk`,
`VhostVsock`, `DeviceVirtioNet`, `DeviceVirtioSerial`, `DeviceVirtconsole`,
`DeviceRng`, `DeviceIntelIommu`, and `DeviceVirtioScsi`.

### 2. Stringly-typed flat `Machine` struct

```rust
struct Machine {
    r#type: String,          // "q35", "virt", "s390-ccw-virtio", …
    accel: String,
    options: String,         // raw accelerator string from config
    nvdimm: bool,
    kernel_irqchip: Option<String>,
    confidential_guest_support: String,
    memory_backend: Option<String>,
}
```

All machine types share one struct.  `kernel_irqchip` is meaningless on `virt`
or `s390x`; `gic-version` is meaningful only on `virt`.  There is no type-level
enforcement.

### 3. No hugepages / NUMA-aware memory backends

`runtime-rs` does not support `memory-backend-file` with `hugepages` today
(tracked in [#12125](https://github.com/kata-containers/kata-containers/issues/12125)).
`MemoryBackendFile` exists in `cmdline_generator.rs` but is only wired for
`/dev/shm` (virtiofs shared memory) and nvdimm paths — never for the
`/hugepages` mount needed to back the entire guest with huge pages.

---

## Target Architecture

The refactor replaces the flat struct with a **machine-centric** model.  All
topology decisions (PCIe bus assignment, IOMMU wiring, NUMA layout, memory
backends) are made when constructing the `Machine` and `Platform`.  Devices
receive a pre-resolved bus string and typed references to shared objects;
they no longer branch on architecture.

### Module layout (target)

Phase 0 introduces the `machine/` directory with all files below.
`platform.rs` and `topology.rs` contain stubs only; `probe.rs` defines
`HostTopology` but not `PlatformProbe` (that trait lands in Phase 1).

```
src/runtime-rs/crates/hypervisor/src/qemu/
├── ARCHITECTURE.md        ← this file
├── cmdline_generator.rs   ← legacy; shrinks as phases complete
├── inner.rs
├── mod.rs
├── qmp.rs
└── machine/               ← Phase 0: all files below introduced here
    ├── mod.rs
    ├── platform.rs        (Platform, Machine, Objects — stubs in Phase 0)
    ├── topology.rs        (PciTopology, PciRootComplex — stubs in Phase 0)
    ├── probe.rs           (HostTopology; PlatformProbe trait is Phase 1)
    ├── q35.rs
    ├── virt.rs
    ├── pseries.rs
    ├── s390x.rs
    └── tests.rs
```

### Core types

#### `Machine` — per-machine-type structs

```rust
pub enum Machine {
    Q35(machine::Q35),
    Virt(machine::Virt),
    Pseries(machine::Pseries),
    S390xCcwVirtio(machine::S390xCcwVirtio),
}

// Q35: intel-iommu is a global singleton on the machine, not per-bus.
pub struct Q35 {
    pub base: BaseMachine,
    pub kernel_irqchip: Option<String>,
    /// Global IOMMU device for Q35.  Emitted as a top-level -device intel-iommu,
    /// not attached to any pxb-pcie.  Contrast with SmmuV3 on PciRootComplex.
    pub intel_iommu: Option<IntelIommuConfig>,
    // pub runtime: RuntimeFeatures,  -- Phase 3+
}

pub struct IntelIommuConfig {
    pub intremap: bool,
    pub caching_mode: bool,
}

// Virt: arm-smmuv3 is per-bus; it lives on PciRootComplex, not here.
pub struct Virt {
    pub base: BaseMachine,
    pub gic_version: Option<u8>,
    pub ras: bool,
    /// Required for Grace GPU passthrough; must be a power of 2.
    /// 4T for GH200/GB200 (≤4 GPUs), 8T for GB300 NVL72 (4 GPUs).
    /// bytes.
    pub highmem_mmio_size: Option<u64>,
    // pub runtime: RuntimeFeatures,  -- Phase 3+
}
```

`kernel_irqchip` and `intel_iommu` live exclusively on `Q35`; `gic_version` and
`highmem_mmio_size` live exclusively on `Virt`.  The compiler enforces this —
no runtime guards needed.

#### `PciTopology` — bus resolution moved here

```rust
pub struct PciTopology {
    pub default_bus: Option<String>,    // "pcie.0" when NUMA / multi-RC is active
    pub roots: Vec<PciRootComplex>,
}

pub struct PciRootComplex {
    pub id: String,              // "pcie.N"
    pub bus_nr: u8,
    /// Maps to pxb-pcie `numa_node=N`.  Required on Grace; omitting it causes
    /// "Unknown NUMA node; performance will be reduced" in the guest kernel.
    pub numa_node: Option<u32>,
    /// Bus-attached IOMMU (arm-smmuv3 on aarch64).  Intel IOMMU is a global
    /// device on Q35 and lives on Machine::Q35, not here.
    pub iommu: Option<BusIommu>,
    /// One entry per passthrough device on this SMMU.
    /// 1 root port = 1 GPU (1:1 SMMU mapping).
    /// N root ports = N GPUs sharing the same physical SMMU.
    pub root_ports: Vec<PciRootPort>,
}

pub struct PciRootPort {
    pub id: String,       // "pcie.portN"
    pub chassis: u8,
    pub device: Option<VfioDevice>,
}

/// IOMMU that attaches to a specific PCIe expander bus (pxb-pcie).
/// Intel IOMMU is a Q35-global device and is NOT represented here —
/// see Machine::Q35::intel_iommu.
pub enum BusIommu {
    SmmuV3 {
        accel: bool,
        ats: bool,
        pasid: bool,
        oas: u8,
        ril: bool,
        /// Enable SMMU command-queue virtualisation (vCMDQ).  Requires
        /// physically contiguous guest memory (hugepages or EGM).
        cmdqv: bool,
    },
}
```

**SMMU grouping rule:** GPUs that share a physical SMMU on the host **must** be
placed on the same `PciRootComplex` in the guest (they share the same
`arm-smmuv3` device).  The IOMMU group boundaries in host sysfs determine the
grouping.  See [Config 3](#config-3--4-gpus-2-gpus-per-smmu-33-numa-nodes) for
the 2-GPUs-per-SMMU topology.

#### `Objects` — shared QEMU `-object` backends

```rust
pub struct Objects {
    pub iommufd: Option<IommufdBackend>,
    pub memory_backends: Vec<MemoryBackend>,
    pub thread_contexts: Vec<ThreadContext>,
    pub acpi_links: Vec<AcpiPciNodeLink>,
    pub rng: Option<ObjectRngRandom>,
}

pub enum MemoryBackend {
    Ram  { id: String, size: u64 },
    /// File-backed memory.  Two uses:
    ///   path = "/dev/hugepages/" → hugepages-backed guest RAM (vCMDQ)
    ///   path = "/dev/egmN"      → per-socket EGM region (vEGM)
    File { id: String, size: u64, path: String, prealloc: bool, share: bool },
}

pub enum AcpiPciNodeLink {
    /// Emitted 8× per passthrough GPU.  The GPU driver uses these nodes to
    /// online GPU memory to the guest kernel (required for MIG regardless of
    /// whether MIG is actually enabled).
    GenericInitiator { id: String, pci_dev: String, node: u32 },
    /// Emitted 1× per passthrough GPU.  Links the GPU to the per-socket EGM
    /// memory-backend file.  `node` is the CpuMem NUMA node for the socket
    /// that holds this GPU's EGM device, not a GPU initiator node.
    EgmMemory        { id: String, pci_dev: String, node: u32 },
}
```

**EGM is per socket, not per GPU:** one `MemoryBackend::File` with `/dev/egmN`
per CPU socket.  Two GPUs on the same socket share the full EGM backing; each
gets its own `acpi-egm-memory` pointing to that socket's CpuMem NUMA node.
See [Config 7](#config-7--vegm-2-gpus-per-socket-4-gpus-2-sockets).

#### `HostTopology` — probe result driving `Platform`

```rust
/// Read from host sysfs and IOMMU group layout before constructing Platform.
pub struct HostTopology {
    pub sockets: Vec<SocketInfo>,
    pub gpu_smmu_groups: Vec<GpuSmmuGroup>,
    pub egm_sockets: Vec<EgmSocketInfo>,
}

pub struct SocketInfo {
    pub id: u32,
    pub cpu_range: std::ops::Range<u32>,
}

/// All GPUs in this group share a physical SMMU and must be placed on the same
/// pxb-pcie + arm-smmuv3 in the guest.  Derived from /sys/kernel/iommu_groups.
pub struct GpuSmmuGroup {
    pub pci_bus_addrs: Vec<String>,   // e.g. ["0008:06:00.0", "0009:06:00.0"]
    pub socket: u32,
}

/// One entry per /dev/egmN device (created by the nvgrace-egm kernel module).
pub struct EgmSocketInfo {
    pub path: String,    // "/dev/egm4"
    pub socket: u32,
    pub total_size: u64,
}
```

`Platform::apply_host_defaults(topo)` consumes `HostTopology` to populate
`PciTopology::roots` (one `PciRootComplex` per `GpuSmmuGroup`) and
`Objects::memory_backends` (one `MemoryBackend::File` per `EgmSocketInfo`).
This is the **only** location that knows about DGX, GB300, or any host flavour.

#### `Platform` — single wiring point

```rust
pub struct Platform {
    pub machine: Machine,
    pub pci: PciTopology,
    pub objects: Objects,
}

impl Platform {
    pub fn from_config(config: &HypervisorConfig) -> Result<Platform> { … }
    pub fn apply_host_defaults(&mut self, topo: &HostTopology) { … }
    pub fn with_hugepages(mut self, path: &str) -> Self { … }
}
```

### NUMA Layout Rules

The guest Linux kernel processes ACPI SRAT entries in a fixed order:

1. **CPU Affinity** — nodes with a `cpus=` range (CpuMem nodes, one per socket)
2. **Generic Affinity** — initiator nodes for PCIe devices (8 per GPU for MIG)
3. **Memory-only Affinity** — nodes without CPUs (EGM backing, hotplug regions)

The `-numa node` arguments **must appear in this order** in the QEMU command
line.  Placing Generic Affinity nodes before CpuMem nodes causes the kernel to
assign wrong NUMA node IDs.

**8 NUMA nodes per GPU (MIG):** Each passthrough GPU requires exactly 8 dedicated
generic-initiator NUMA nodes regardless of whether MIG is in use.  The GPU
driver (CUDA) uses these nodes to online GPU memory to the guest kernel.  Total
node count with 4 GPUs on a single-socket host: 1 CpuMem + 4 × 8 = 33 nodes.

**GPU memory spill prevention:** GPU NUMA nodes may attract page migration from
`autonuma` or systemd NUMA policies.  Mitigate with explicit NUMA distances:

```text
-numa dist,src=<gpu_node>,dst=<cpumem_node>,val=254
```

Or by disabling NUMA balancing in the guest OS.

**`highmem-mmio-size` sizing on `-machine virt`:**
- GH200 / GB200 with ≤ 4 GPUs → `4T`
- GB300 NVL72 with 4 GPUs → `8T`
- Must be a power of 2; round up to the next power when in doubt.

### Hugepages wiring (`with_hugepages`)

```rust
pub fn with_hugepages(mut self, path: &str) -> Self {
    let id = "m0".to_owned();
    self.objects.memory_backends.push(MemoryBackend::File {
        id: id.clone(),
        size: self.machine.memory_size(),
        path: path.to_owned(),   // "/dev/hugepages/"
        prealloc: true,
        share: true,
    });
    self.machine.set_memory_backend(&id);  // -machine memory-backend=m0
    self
}
```

This replaces the current pattern where `set_memory_backend_file` +
`set_memory_backend` are called ad-hoc from `QemuCmdLine` internals.  The
caller checks `config.memory_info.enable_hugepages` and calls
`Platform::with_hugepages("/dev/hugepages/")` once.  No device code changes.

EGM memory is natively physically contiguous, so vEGM implicitly satisfies the
vCMDQ contiguity requirement without hugepages.  `cmdqv=on` is still required
in the `arm-smmuv3` device args when vCMDQ is enabled.

### Emission order

`QemuCmdLine::build()` becomes a thin orchestrator:

1. Emit `-object iommufd,id=iommufd0` first (all vfio devices and SMMU SID
   tables reference it).
2. Emit remaining `Objects` — `memory_backends`, `thread_contexts`, `rng` —
   all `-object` lines, IDs defined before any reference.
3. Emit `Machine` — picks up `memory-backend=` from objects, `highmem-mmio-size`.
4. Emit **CpuMem** `-numa node` entries: one per socket, with `cpus=` + `memdev=`.
5. Emit **GPU initiator** `-numa node` entries: 8 per GPU, no `cpus`/`memdev`,
   ordered by GPU index.
6. Emit **EGM / hotplug** `-numa node` entries: memory-only nodes, no `cpus`.
7. Emit `PciTopology` in bus-number order: `pxb-pcie` (with `numa_node=`),
   `arm-smmuv3`, root ports, vfio devices per root complex.
8. Emit `Objects.acpi_links`: `acpi-generic-initiator` (8 per GPU) then
   `acpi-egm-memory` (1 per GPU).  These reference device IDs emitted in step 7.

Steps 4–6 must be in that order to match Linux ACPI SRAT processing.

---

## Grace Platform Configurations

The 7 configurations below are derived from tested production deployments of
NVIDIA Grace GPU passthrough.  Each becomes a golden test fixture in **Phase 0b**.
The implementation must reproduce every one exactly from the corresponding
`Platform` + `HostTopology` input.

All Grace configurations share these constants:
- `-device vfio-pci-nohotplug` (not `vfio-pci`) — required for C2C interconnect
- `-object iommufd,id=iommufd0` — modern IOMMU fd interface; legacy VFIO groups
  not supported on Grace
- `arm-smmuv3` fixed parameters: `accel=on,ats=on,ril=off,pasid=on,oas=48`
- Host kernel driver: `nvgrace-gpu-vfio-pci` (replaces standard `vfio-pci`)
- EGM kernel module: `nvgrace-egm` (creates `/dev/egm*` character devices)

### Config 1 — Single GPU, 1 SMMU (9 NUMA nodes)

```text
-object iommufd,id=iommufd0
-object memory-backend-ram,size=16G,id=m0
-machine virt,accel=kvm,gic-version=3,ras=on,highmem-mmio-size=4T,memory-backend=m0
-numa node,memdev=m0,cpus=0-3,nodeid=0
-numa node,nodeid=1
...
-numa node,nodeid=8
-device pxb-pcie,id=pcie.1,bus_nr=1,bus=pcie.0,numa_node=0
-device arm-smmuv3,primary-bus=pcie.1,id=smmuv3.1,accel=on,ats=on,ril=off,pasid=on,oas=48
-device pcie-root-port,id=pcie.port1,bus=pcie.1,chassis=1,io-reserve=0
-device vfio-pci-nohotplug,host=0008:06:00.0,bus=pcie.port1,rombar=0,id=dev0,iommufd=iommufd0
-object acpi-generic-initiator,id=gi0,pci-dev=dev0,node=1
...
-object acpi-generic-initiator,id=gi7,pci-dev=dev0,node=8
```

`HostTopology`: 1 socket, 1 `GpuSmmuGroup { pci_bus_addrs: ["0008:06:00.0"], socket: 0 }`.

### Config 2 — 4 GPUs, 1 GPU per SMMU (33 NUMA nodes)

Each GPU gets its own `PciRootComplex` (one `pxb-pcie` + one `arm-smmuv3` + one
root port).  Repeat the pxb-pcie/smmuv3/root-port/vfio block 4 times:

```text
-object iommufd,id=iommufd0
-object memory-backend-ram,size=16G,id=m0
-machine virt,...,highmem-mmio-size=4T,memory-backend=m0
-numa node,memdev=m0,cpus=0-3,nodeid=0
-numa node,nodeid=1 ... -numa node,nodeid=32   # 4×8 = 32 GPU initiator nodes

# Per GPU (N = 1..4):
-device pxb-pcie,id=pcie.N,bus_nr=N,bus=pcie.0,numa_node=0
-device arm-smmuv3,primary-bus=pcie.N,id=smmuv3.N,accel=on,ats=on,ril=off,pasid=on,oas=48
-device pcie-root-port,id=pcie.portN,bus=pcie.N,chassis=N,io-reserve=0
-device vfio-pci-nohotplug,host=<addr>,bus=pcie.portN,rombar=0,id=dev<N-1>,iommufd=iommufd0
-object acpi-generic-initiator,id=gi<8*(N-1)>,pci-dev=dev<N-1>,node=<1+8*(N-1)>
...                                                               # ×8 per GPU
```

`HostTopology`: 1 socket, 4 `GpuSmmuGroup` each with 1 address.

### Config 3 — 4 GPUs, 2 GPUs per SMMU (33 NUMA nodes)

GPUs sharing a physical SMMU share one `PciRootComplex` with **2 root ports**.
2 complexes × 2 GPUs each:

```text
-device pxb-pcie,id=pcie.1,bus_nr=1,bus=pcie.0,numa_node=0
-device arm-smmuv3,primary-bus=pcie.1,id=smmuv3.1,accel=on,ats=on,ril=off,pasid=on,oas=48
-device pcie-root-port,id=pcie.port1,bus=pcie.1,chassis=1,io-reserve=0
-device vfio-pci-nohotplug,host=0008:06:00.0,bus=pcie.port1,rombar=0,id=dev0,iommufd=iommufd0
-device pcie-root-port,id=pcie.port2,bus=pcie.1,chassis=2,io-reserve=0
-device vfio-pci-nohotplug,host=0009:06:00.0,bus=pcie.port2,rombar=0,id=dev1,iommufd=iommufd0

-device pxb-pcie,id=pcie.2,bus_nr=9,bus=pcie.0,numa_node=0
-device arm-smmuv3,primary-bus=pcie.2,id=smmuv3.2,accel=on,ats=on,ril=off,pasid=on,oas=48
-device pcie-root-port,id=pcie.port3,bus=pcie.2,chassis=3,io-reserve=0
-device vfio-pci-nohotplug,host=0010:06:00.0,bus=pcie.port3,rombar=0,id=dev2,iommufd=iommufd0
-device pcie-root-port,id=pcie.port4,bus=pcie.2,chassis=4,io-reserve=0
-device vfio-pci-nohotplug,host=0011:06:00.0,bus=pcie.port4,rombar=0,id=dev3,iommufd=iommufd0

-object acpi-generic-initiator,id=gi0,pci-dev=dev0,node=1
...
-object acpi-generic-initiator,id=gi7,pci-dev=dev0,node=8
-object acpi-generic-initiator,id=gi8,pci-dev=dev1,node=9
...
-object acpi-generic-initiator,id=gi15,pci-dev=dev1,node=16
# × 2 more for dev2, dev3
```

`HostTopology`: 1 socket, 2 `GpuSmmuGroup` each with 2 addresses.

### Config 4 — GPU + NIC passthrough

Same structure as Config 2 but one `PciRootPort` holds a NIC (`vfio-pci-nohotplug`
with the NIC's PCI address).  That root port does **not** emit
`acpi-generic-initiator` links — the NIC has no GPU memory to online.

`VfioDevice` carries a `kind: VfioDeviceKind` field (enum `Gpu` / `Nic` / …)
that gates initiator emission.  The NIC shares the host SMMU with no GPU on its
bus, so it gets its own `PciRootComplex`.

### Config 5 — vCMDQ (hugepages + SMMU command-queue virtualisation)

Same PCIe topology as Config 1 or 2, but `MemoryBackend::Ram` is replaced with
`MemoryBackend::File` for physically contiguous memory (required by the vCMDQ
hardware for the queue base address), and `cmdqv=on` is added to `arm-smmuv3`:

```text
-object memory-backend-file,id=m0,size=16G,mem-path=/dev/hugepages/,prealloc=on,share=on
-machine virt,...,memory-backend=m0
-device arm-smmuv3,...,cmdqv=on
```

`Platform::with_hugepages("/dev/hugepages/")` + `IommuKind::SmmuV3 { cmdqv: true }`.

### Config 6 — vEGM, 1 GPU per socket (4 GPUs, 4 sockets)

One `memory-backend-file` per socket using the `/dev/egmN` device created by
`nvgrace-egm`.  One `acpi-egm-memory` per GPU pointing to its socket's CpuMem
NUMA node:

```text
-object memory-backend-file,id=m0,mem-path=/dev/egm4,size=56896M,share=on,prealloc=on
-object memory-backend-file,id=m1,mem-path=/dev/egm5,size=56896M,share=on,prealloc=on
-object memory-backend-file,id=m2,mem-path=/dev/egm6,size=56896M,share=on,prealloc=on
-object memory-backend-file,id=m3,mem-path=/dev/egm7,size=56896M,share=on,prealloc=on
-machine virt,...
-numa node,memdev=m0,cpus=0,nodeid=0
-numa node,memdev=m1,cpus=1,nodeid=1
-numa node,memdev=m2,cpus=2,nodeid=2
-numa node,memdev=m3,cpus=3,nodeid=3
-numa node,nodeid=4 ... -numa node,nodeid=35   # 4×8 GPU initiator nodes

# PCI topology: 4× (pxb-pcie + smmuv3 + root-port + vfio) — same shape as Config 2

-object acpi-egm-memory,id=egm0,pci-dev=dev0,node=0   # GPU on socket 0
-object acpi-egm-memory,id=egm1,pci-dev=dev1,node=1   # GPU on socket 1
-object acpi-egm-memory,id=egm2,pci-dev=dev2,node=2
-object acpi-egm-memory,id=egm3,pci-dev=dev3,node=3
```

`HostTopology`: 4 sockets, 4 `GpuSmmuGroup` (1 GPU each), 4 `EgmSocketInfo`.

### Config 7 — vEGM, 2 GPUs per socket (4 GPUs, 2 sockets)

Two GPUs per socket share the socket's EGM device.  The `/dev/egmN` path appears
in one `memory-backend-file` at full socket size.  Both `acpi-egm-memory` entries
for that socket point to the same CpuMem NUMA node:

```text
-object memory-backend-file,id=m0,mem-path=/dev/egm4,size=56896M,share=on,prealloc=on
-object memory-backend-file,id=m1,mem-path=/dev/egm5,size=56896M,share=on,prealloc=on
-machine virt,...
-numa node,memdev=m0,cpus=0-1,nodeid=0
-numa node,memdev=m1,cpus=2-3,nodeid=1
-numa node,nodeid=2 ... -numa node,nodeid=33   # 4×8 GPU initiator nodes

# PCI topology: 2× (pxb-pcie + smmuv3 + 2 root ports + 2 vfio) — Config 3 shape

-object acpi-egm-memory,id=egm0,pci-dev=dev0,node=0   # both GPUs on socket 0 → node=0
-object acpi-egm-memory,id=egm1,pci-dev=dev1,node=0
-object acpi-egm-memory,id=egm2,pci-dev=dev2,node=1   # both GPUs on socket 1 → node=1
-object acpi-egm-memory,id=egm3,pci-dev=dev3,node=1
```

`HostTopology`: 2 sockets, 2 `GpuSmmuGroup` (2 GPUs each), 2 `EgmSocketInfo`.

---

## Migration Phases

Each phase is a self-contained PR.  Phases 0–1 introduce new types without
touching the hot path; Phases 2–5 strangle the old code one device at a time.

### Phase 0 — Test harness and empty data types

- **0a** — golden-test harness + one trivial fixture (basic `virt` machine)
- **0b** — All 7 Grace configurations as command-line fixtures + parse smoke test.
  Each fixture provides the expected QEMU argument list and the `HostTopology`
  input that produces it.  Zero implementation; tests all fail intentionally.
- **0c** — empty `machine/` module with unit tests on pure helpers
  (`format_memory`, `numa_node` string, bus-name helpers, etc.)

No behaviour changes.  CI green throughout.

### Phase 1 — Platform probe (unused)

Introduce `PlatformProbe` trait, `HostTopology` struct, and `Platform::from_config`.
Nothing in the hot path calls them yet.  Tests assert construction succeeds for
each supported machine type and that `HostTopology` round-trips through
`apply_host_defaults` without panic.

### Phase 2 — Strangle bus resolution (one device per PR)

For each device listed below, pass a resolved `bus: String` instead of
computing it inside `ToQemuParams`:

- `DeviceVhostUserFs`
- `DeviceVirtioBlk`
- `VhostVsock`
- `DeviceVirtioNet`
- `DeviceVirtioSerial`
- `DeviceVirtconsole`
- `DeviceRng`
- `DeviceIntelIommu`
- `DeviceVirtioScsi`

Each PR removes one `#[cfg(target_arch)]` block and one duplicated comment.
The golden fixtures validate that the emitted command lines are unchanged.

Final PR in Phase 2: remove all remaining `#[cfg(target_arch)]` blocks from
`cmdline_generator.rs`.

### Phase 3 — Objects registry

- Lift `MemoryBackendFile` into `Objects::memory_backends` as
  `MemoryBackend::File`.
- Wire hugepages via `Platform::with_hugepages`.
- Lift `ObjectIoThread` / `ObjectRngRandom` into `Objects`.

This is the phase that enables hugepages for `runtime-rs` (issue #12125).

### Phase 4 — Multi-RC PCIe and NUMA layout

- Emit `pxb-pcie` (with `numa_node=`) + per-RC `arm-smmuv3` from `PciTopology`.
- Support N root ports per `PciRootComplex` (Config 3 shape: 2 GPUs per SMMU).
- Add `VfioPciNoHotplug` with typed `IommufdRef` and `VfioDeviceKind`.
- Emit NUMA nodes in the correct order (CpuMem → GPU initiators → memory-only).
- `apply_host_defaults` wired end-to-end: Configs 1–4 golden fixtures pass.

### Phase 5 — vCMDQ and vEGM

- `SmmuV3 { cmdqv: true }` + `MemoryBackend::File { path: "/dev/hugepages/", … }`.
- `AcpiPciNodeLink::EgmMemory` + per-socket `MemoryBackend::File { path: "/dev/egmN", … }`.
- `EgmSocketInfo` probe wired into `apply_host_defaults`.
- Configs 5–7 golden fixtures pass.

### Phase 6 — Cleanup

- Delete dead code from `cmdline_generator.rs`.
- Remove feature flags / compat shims introduced during migration.
- Final golden-test sweep across all 7 Grace configs plus existing machine types.
- Remove all TODOs and decision stubs from this document.

---

## Platform Parity

The `Platform` type must generate correct QEMU command lines for every supported
machine type, not just Grace/virt.  The legacy `Machine` struct carried two raw
string fields that have no typed home yet:

| Field                        | Legacy value                         | Machine types     |
|------------------------------|--------------------------------------|-------------------|
| `accel`                      | `"kvm"`, `"tcg"`, `"kvm:tcg"`       | all               |
| `options`                    | raw KVM accelerator options          | x86/arm           |
| `kernel_irqchip`             | `"on"`, `"split"`, `"off"`           | Q35 only          |
| `confidential_guest_support` | `"sev-snp0"`, `"tdx0"`, `""`        | Q35 / virt        |
| `memory_backend`             | e.g. `"m0"`                          | virt (NUMA/EGM)   |

These are preserved as raw strings in `BaseMachine` for now.  Phase 3 will
introduce typed representations.

### Baseline `-machine` output per type

The examples below show the minimum expected output for vanilla (no GPU) configs
derived from tested deployments.  They anchor the per-machine fixture set.

**virt (aarch64 — vanilla kata)**
```text
-machine virt,accel=kvm,gic-version=3,ras=on
```

**Q35 (x86_64 — vanilla kata)**
```text
-machine q35,accel=kvm
```
(`kernel-irqchip` is absent on vanilla Q35; it is only required for CoCo.)

**Q35 + TDX (x86_64 — CoCo)**
```text
-object tdx-guest,id=tdx,...
-machine q35,accel=kvm,kernel-irqchip=split,confidential-guest-support=tdx
```

**Q35 + SEV-SNP (x86_64 — CoCo)**
```text
-object sev-snp-guest,id=sev-snp,...
-machine q35,accel=kvm,kernel-irqchip=split,confidential-guest-support=sev-snp
```

**s390-ccw-virtio (s390x)**
```text
-machine s390-ccw-virtio,accel=kvm
```

Both `machine_accelerators` (the raw KVM option string) and
`confidential_guest_support` need typed representations before the legacy
`Machine` struct can be deleted.  Tracked in Phase 3.

---

## Planned Fixture Configurations

The 7 Grace fixtures cover Grace GPU passthrough thoroughly.  The configurations
below must also be captured as golden fixtures before Phase 6 closes.  Each
entry notes the data source required: fixture content must come from actual
production QEMU invocations, not from documentation.

### Vanilla kata — virt (aarch64)

Basic `virt` machine with no GPU passthrough, no NUMA, no hugepages.
Represents the common ARM64 kata use-case.

**Data needed:** capture `qemu-system-aarch64` invocation from a running
non-GPU kata pod on an ARM64 host.

### Vanilla kata — Q35 (x86_64)

**Production data captured** (DGX x86 host, 2026-07-07).
Fixture: `q35_vanilla_kata_x86.args`.  Test: `q35_vanilla_kata_x86` (ignored, Phase 3).

Key observations from the production invocation:

- `-machine q35,accel=kvm` — no `kernel-irqchip` on vanilla; only required for CoCo
- NUMA memory model differs from Grace: total memory via `-m 73728M,slots=10,maxmem=127052M`;
  NUMA pinning via separate `memory-backend-file` objects with `host-nodes=N,policy=bind`
  backed by `/dev/shm` (not `/dev/hugepages` or `/dev/egm*`)
- Two NUMA nodes: socket 0 cpus 0-32 / 36864M, socket 1 cpus 33-65 / 36864M;
  distance 20 between them
- 8 `pcie-root-port` pre-provisioned on `pcie.0` (slots 0-7) for GPU cold-plug
  (`cold_plug_vfio=root-port`, `pcie_root_port=8`, `hot_plug_vfio=no-port` in
  `configuration-qemu-nvidia-gpu.toml.in`); GPU VFIO devices are added to the
  static QEMU command line before the VM boots, not via QMP after boot
- No `pxb-pcie`, no `arm-smmuv3` — Q35 GPU passthrough uses cold-plug onto `pcie.0`
  root ports, not the static vfio-pci-nohotplug topology used on Grace

New Platform fields required (Phase 3):
- `MemoryBackend::File { host_nodes: Option<u32>, policy: Option<String> }` for NUMA SHM
- `Objects::numa_distances: Vec<(u32, u32, u32)>` for `-numa dist` entries
- `HostTopology` additions for NUMA SHM paths and distances

### CoCo + GPU passthrough (SEV-SNP or TDX)

**SEV-SNP production data captured** (AMD EPYC host, 2026-07-13).
Fixture: `q35_coco_snp_single_gpu.args`.  Test: `q35_coco_snp_single_gpu` (ignored, Phase 3).

Key observations from the SEV-SNP + GPU invocation:

- `-object sev-snp-guest,id=snp,cbitpos=51,reduced-phys-bits=1,kernel-hashes=on,policy=196608,host-data=...`
  emitted BEFORE the `-machine` line (QEMU requires the protection object first)
- `-machine q35,accel=kvm,kernel_irqchip=split,confidential-guest-support=snp`
  (underscore in `kernel_irqchip`; `split` is required for SNP/TDX, not `on`)
- Memory: `memory-backend-ram` with `host-nodes=N,policy=bind` for NUMA pinning;
  CoCo uses RAM backend (not file-backed `/dev/shm`) — single NUMA node
- GPU passthrough via `pxb-pcie + pcie-root-port + vfio-pci` (same shape as Grace
  but `vfio-pci` NOT `vfio-pci-nohotplug`, no `arm-smmuv3` — x86 uses global IOMMU)
- iommufd is **per-device** (`id=iommufdvfio-<uuid>`), NOT the shared `iommufd0`
  used on Grace; one iommufd object per GPU
- `x-pci-vendor-id=0x10de,x-pci-device-id=0x2321` overrides required so the guest
  sees the correct device IDs for measured boot / attestation
- `pxb-pcie bus_nr=32` (not the Grace 1-indexed cumulative formula)
- BIOS: `AMDSEV.fd` (AMD-specific OVMF build, not generic `OVMF.fd`)
- Binary: `qemu-system-x86_64-snp-experimental` (patched QEMU for SNP support)

New Platform fields required (Phase 3):
- `Objects::protection: Option<ProtectionDevice>` (`sev-snp-guest` / `tdx-guest`)
- `Q35::kernel_irqchip: Option<String>` typed field (`"split"` for CoCo, absent for vanilla)
- `Q35::confidential_guest_support: Option<String>` referencing the protection object id
- `MemoryBackend::Ram { host_nodes: Option<u32>, policy: Option<String> }` for NUMA pinning
- Per-device iommufd: `PciRootComplex::iommufd: Option<IommufdRef>` (not shared)
- `VfioDevice::pci_vendor_id / pci_device_id: Option<u16>` for CoCo attestation overrides

**TDX data still needed:** capture from a CoCo + GPU pod on an Intel TDX host.

### 8 GPUs + 4 NVSwitches (DGX/HGX topology)

NVSwitch passthrough adds a new device kind and a multi-level PCIe hierarchy
not present in the Grace configs:

- `VfioDeviceKind::NvSwitch` is not yet defined in `topology.rs` (only `Gpu`
  and `Nic` exist).
- NVSwitches currently use `VfioDeviceConfig` (not `VfioDeviceGroup`) in the
  legacy path (`add_gpu_nvswitch_setup` at cmdline_generator.rs:3373).
- PCIe hierarchy: root port → `x3130-upstream` → `xio3130-downstream` →
  device (three levels vs. the two levels used for GPU direct attachment).
- `add_pcie_switch_ports` (cmdline_generator.rs:3508) emits this hierarchy;
  `PciTopology` has no equivalent typed representation yet.

New types needed before a fixture can be written:
- `VfioDeviceKind::NvSwitch`
- `PciSwitchPort { upstream: PcieUpstreamPort, downstream: Vec<PcieDownstreamPort> }` on `PciRootComplex`
- `HostTopology::nvswitch_addrs` or equivalent probe field

**Data needed:** capture from a DGX/HGX or GB200 NVL system with 8 GPUs and
4 NVSwitches passed through.  Exact bus_nr arithmetic and PCIe address
assignments must come from a live invocation, not from inference.

---

## Known Issues and Follow-up Items

Misconfiguration and known defects in the current runtime-rs/QEMU path that
are related to this refactor.  Items marked **post-refactor** require the new
`Platform` emission path to be wired end-to-end before they can be addressed
cleanly.

### 4+ Blackwell (B200/B300) GPU passthrough fails due to 40-bit GPA cap (#13270)

`-cpu host` without `host-phys-bits=on` limits the guest physical address space
to ~40 bits (~1 TiB).  Each B200/B300 GPU has a 128 GiB 64-bit prefetchable BAR;
three GPUs (384 GiB) fit within 1 TiB, but four GPUs (512 GiB) do not, causing
the fourth GPU to fail PCIe BAR assignment at VM boot.

**Fix:** set `cpu_features = "pmu=off,host-phys-bits=on"` in
`configuration-qemu-nvidia-gpu*.toml.in`.  The `Makefile` now defaults
`CPUFEATURES` and `TDXCPUFEATURES` to `pmu=off,host-phys-bits=on` for `amd64`
builds so operator installs that regenerate configs from source pick up the fix;
existing installed configs must be updated manually.

**Post-refactor:** the `Platform` Q35 builder should emit `host-phys-bits=on`
unconditionally for x86_64/KVM so the fix is durable even if the config file
is overwritten by `kata-deploy`.

### QMP startup timeout is hard-coded and ignores the caller's deadline (#13343)

`QemuInner::start_vm(_timeout)` accepts the timeout argument from the
`Hypervisor` trait but silently ignores it.  QMP initialisation instead uses
two independent hard-coded values:

- 5 s per-read socket timeout (`QMP_SOCKET_TIMEOUT`)
- 50 s overall connect/init deadline (`QMP_INIT_TIMEOUT`)

Under large-memory / VFIO / GPU passthrough conditions QEMU can take longer
than 50 s before QMP is fully responsive, causing a spurious timeout failure
even though the higher-level `start_vm` caller's intended budget was never
applied.

The `Hypervisor::start_vm(timeout: i32)` contract is also inconsistently
interpreted across backends: QEMU and Firecracker ignore it, Dragonball
treats it as milliseconds (despite comments saying seconds), Cloud Hypervisor
treats it as seconds, and Remote treats it as seconds for the RPC deadline.

**Minimum fix for GPU passthrough (pre-refactor):** make `QMP_INIT_TIMEOUT`
configurable via `HypervisorConfig` (e.g. `qmp_init_timeout_secs`) so
operators can raise it without patching.

**Post-refactor:** once `Platform::to_qemu_args` drives QEMU startup, wire
the QMP connect/init step to consume the caller's remaining `start_vm` budget
rather than maintaining an independent fixed deadline.

### `seccomp_sandbox` (-sandbox) not plumbed through Platform

Pre-refactor the option works via the legacy `cmdline_generator.rs` path.
Post-refactor it needs a typed `Objects::seccomp_sandbox` field so the legacy
generator can be removed.  Tracked in Phase 3.

### `machine_accelerators` and `confidential_guest_support` have no typed home

Tracked in the Platform Parity section.  Both are passed as raw strings
through `BaseMachine` today; they need typed representations before the
legacy `Machine` struct can be deleted.

---

## Design Principles

1. **Machine decisions stay in `Machine` and `PciTopology`.**
   Devices receive resolved values; they do not branch on architecture or
   machine type.

2. **Singletons are first-class.**
   `iommufd0` is emitted once and referenced via `IommufdRef`.  No string
   concatenation in device impls.

3. **One location for host-specific wiring.**
   `Platform::apply_host_defaults` is the only place that knows about DGX,
   GB300, or any future host flavour.

4. **IOMMU placement matches hardware placement.**
   Bus-attached IOMMUs (`arm-smmuv3`) live on `PciRootComplex`.  Global IOMMUs
   (`intel-iommu`) live on the machine type (`Machine::Q35`).  These are
   different device placement models and must not share a field.
   SMMU grouping: devices sharing a physical SMMU are placed on the same
   `PciRootComplex`; devices on separate physical SMMUs get separate entries.

5. **NUMA emission order is non-negotiable.**
   CpuMem → GenericInitiator → MemoryOnly.  The `Platform` builder enforces
   this by construction; there is no API to emit them in a different order.

6. **Incremental migration, no flag day.**
   Each phase leaves CI green.  Old `QemuCmdLine` and new `Platform` coexist
   until the strangle is complete.

7. **Types enforce constraints.**
   `kernel_irqchip` compiles only on `Q35`.  `cmdqv` compiles only on
   `IommuKind::SmmuV3`.  You cannot emit `gic-version` on a Q35 machine.

---

## Related Documents

- [Issue #12187](https://github.com/kata-containers/kata-containers/issues/12187) — full design spec with data-model definitions and worked examples
- [Issue #12125](https://github.com/kata-containers/kata-containers/issues/12125) — NUMA and hugepages roadmap

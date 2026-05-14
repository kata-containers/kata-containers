# NUMA Support for Kata Containers with QEMU

## Overview

Non-Uniform Memory Access (NUMA) is a memory architecture where access
latency depends on which CPU is accessing which memory region. On
multi-socket or multi-chiplet systems, each NUMA node has local memory that
its CPUs can access faster than remote memory belonging to other nodes.

When running performance-sensitive workloads — particularly GPU passthrough
via VFIO — cross-NUMA memory access can significantly degrade throughput.
Kata Containers can expose the host NUMA topology to the guest VM so that
vCPUs, memory, and devices are all placed on the correct NUMA node, preserving
memory locality.

This guide walks through the full setup end-to-end: host inspection,
Kubernetes configuration, Kata configuration, pod deployment, and
verification.

> **Note:**
>
> NUMA support is currently available only for the **Go runtime** with the
> **QEMU hypervisor** on **amd64** and **arm64** architectures. The Rust
> runtime (`runtime-rs`) does not yet support NUMA topology.

## Step 1: Inspect the Host NUMA Topology

Before configuring anything, understand your host. Run on each worker node:

```bash
$ numactl --hardware
```

Example output on a 2-socket system with 8 CPUs per socket:

```
available: 2 nodes (0-1)
node 0 cpus: 0 1 2 3 4 5 6 7
node 0 size: 65536 MB
node 1 cpus: 8 9 10 11 12 13 14 15
node 1 size: 65536 MB
node distances:
node   0   1
  0:  10  21
  1:  21  10
```

Take note of:
- How many NUMA nodes exist (here: 2)
- Which CPUs belong to each node (here: 0-7 on node 0, 8-15 on node 1)
- The distance matrix (here: 10 local, 21 remote)

If you have GPUs, check which NUMA node each GPU is attached to:

```bash
$ lspci -nnk -d 10de: | grep -A2 "NVIDIA"
$ cat /sys/bus/pci/devices/0000:41:00.0/numa_node
```

Replace `0000:41:00.0` with your GPU's PCI address. The output (`0` or `1`)
tells you which NUMA node the GPU sits on.

On a single-NUMA host (only node 0), enabling NUMA is a harmless no-op —
the runtime detects one node and skips multi-NUMA topology.

## Step 2: Kubernetes CPU Manager Policy

Kata's NUMA-aware vCPU pinning works **without** `cpuManagerPolicy: static`.
The recommended policy is the default (`none`):

```yaml
apiVersion: kubelet.config.k8s.io/v1beta1
kind: KubeletConfiguration
cpuManagerPolicy: "none"
```

> **Why not `static`?**
>
> With `cpuManagerPolicy: static`, Kubernetes assigns dedicated CPUs to
> Guaranteed QoS pods. On a multi-NUMA host, those CPUs are often all from
> a **single** NUMA node (depending on the topology manager policy). This
> causes the sandbox CPUSet to cover only one NUMA node, which defeats the
> purpose of multi-NUMA guest topology.
>
> With `cpuManagerPolicy: none` (the default), the pod inherits the full
> node CPUSet spanning all NUMA nodes, and Kata's NUMA-aware pinning
> distributes vCPU threads proportionally across host NUMA nodes.

### 2.1 Check the current policy

```bash
$ grep cpuManagerPolicy /var/lib/kubelet/config.yaml
```

If it shows `static`, switch to `none`:

```bash
$ sudo sed -i 's/cpuManagerPolicy:.*/cpuManagerPolicy: "none"/' /var/lib/kubelet/config.yaml
$ sudo rm -f /var/lib/kubelet/cpu_manager_state
$ sudo systemctl restart kubelet
```

## Step 3: Configure Kata Containers for NUMA

> **Note:**
>
> If you are using the NVIDIA GPU runtime classes
> (`kata-qemu-nvidia-gpu`, `kata-qemu-nvidia-gpu-snp`,
> `kata-qemu-nvidia-gpu-tdx`), NUMA is already enabled by default in their
> configuration templates. You only need the steps below for the base
> `kata-qemu` runtime class or custom configurations.

Never edit the base `configuration-qemu.toml` directly — use a
**configuration drop-in** so your customizations survive upgrades.

### 3.1 Via kata-deploy Helm chart (recommended)

Add a custom runtime with a NUMA drop-in in your Helm values file:

```yaml
customRuntimes:
  enabled: true
  runtimes:
    numa:
      baseConfig: qemu
      runtimeClass: |
        apiVersion: node.k8s.io/v1
        kind: RuntimeClass
        metadata:
          name: kata-qemu-numa
        handler: kata-qemu-numa
      dropIn: |
        [hypervisor.qemu]
        enable_numa = true
        numa_mapping = []

        [runtime]
        static_sandbox_resource_mgmt = true
        enable_vcpus_pinning = true
```

Then install (or upgrade) the Helm chart:

```bash
$ helm upgrade kata-deploy \
    --namespace kata-system \
    -f my-values.yaml \
    "${CHART}" --version "${VERSION}"
```

Pods using `runtimeClassName: kata-qemu-numa` will get the NUMA-enabled
configuration.

With `numa_mapping = []` (empty), the runtime auto-discovers host NUMA nodes
and creates a 1:1 guest-to-host mapping, then **right-sizes** the resulting
topology: if the sandbox's CPU and memory budget fits on a single host
NUMA node — and any cold-plugged VFIO devices live on that same node —
the guest topology collapses to that one node so the workload keeps full
memory locality without paying a multi-node penalty. Sandboxes that
genuinely span multiple host nodes keep the auto-derived multi-node
topology. An explicit `numa_mapping` opts out of right-sizing and is
honored verbatim — useful when you want a specific layout regardless of
sandbox size, or to group multiple host nodes into fewer guest nodes
(e.g., on a 4-socket system):

```yaml
      dropIn: |
        [hypervisor.qemu]
        enable_numa = true
        numa_mapping = ["0-1", "2-3"]
```

Each entry is a cpuset-style string (ranges like `0-3` and lists like
`0,2,4` are both valid).

### 3.2 Via manual drop-in on the node

If you manage nodes directly (without kata-deploy), create a drop-in file
under the `config.d/` directory. Use a `50-*` prefix (the reserved range
for user customizations):

```bash
$ cat > /opt/kata/share/defaults/kata-containers/runtimes/qemu/config.d/50-numa.toml <<'EOF'
[hypervisor.qemu]
enable_numa = true
numa_mapping = []

[runtime]
static_sandbox_resource_mgmt = true
enable_vcpus_pinning = true
EOF
```

The drop-in is merged on top of the base `configuration-qemu.toml`
automatically. No restart is needed — the shim reads the configuration
at pod creation time.

> **Note:**
>
> For details on the drop-in mechanism, reserved prefix ranges, and
> additional Helm examples, see the
> [Helm configuration guide](../../docs/helm-configuration.md).

### 3.3 Verify the effective configuration

After applying the drop-in, verify the merged configuration on the node:

```bash
$ grep -rE "enable_numa|numa_mapping|static_sandbox_resource_mgmt|enable_vcpus_pinning" \
    /opt/kata/share/defaults/kata-containers/runtimes/qemu/config.d/
```

## Step 4: Deploy a NUMA-Aware Pod

### 4.1 Basic NUMA pod

Create a pod that requests enough CPUs to span both NUMA nodes. Use the
runtime class matching your NUMA configuration from Step 3 (e.g.,
`kata-qemu-numa` if you created a custom runtime, or `kata-qemu` if you
applied a drop-in to the base config). Kata sizes the VM based on
`limits`, so set `limits.cpu` to the desired vCPU count:

```bash
$ cat <<'EOF' | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: numa-test
spec:
  runtimeClassName: kata-qemu-numa
  containers:
  - name: numa-check
    image: ubuntu:24.04
    command: ["sleep", "infinity"]
    resources:
      requests:
        cpu: "1"
        memory: "1Gi"
      limits:
        cpu: "80"
        memory: "64Gi"
EOF
```

> **Note:**
>
> Kata sizes the VM based on `limits` (not `requests`). Using different
> values for `requests` and `limits` makes the pod **Burstable** QoS,
> which avoids Kubernetes CPU manager interference with NUMA-aware
> pinning. The large `limits.cpu` value tells Kata to create a VM with
> that many vCPUs distributed across NUMA nodes.

### 4.2 GPU passthrough pod with NUMA

For GPU workloads, use the NVIDIA GPU runtime class. NUMA is enabled by
default in the GPU configuration templates:

```bash
$ cat <<'EOF' | kubectl apply -f -
apiVersion: v1
kind: Pod
metadata:
  name: gpu-numa-test
spec:
  runtimeClassName: kata-qemu-nvidia-gpu
  containers:
  - name: cuda-test
    image: nvcr.io/nvidia/k8s/cuda-sample:vectoradd-cuda12.5.0-ubuntu22.04
    resources:
      limits:
        cpu: "4"
        memory: "8Gi"
        nvidia.com/pgpu: "1"
EOF
```

## Step 5: Verify NUMA Inside the Guest

### 5.1 Check guest NUMA topology

Exec into the running pod and inspect the NUMA layout:

```bash
$ kubectl exec -it numa-test -- bash
```

Inside the pod:

```bash
$ apt-get update && apt-get install -y numactl
$ numactl --hardware
```

Expected output on a 2-NUMA-node guest:

```
available: 2 nodes (0-1)
node 0 cpus: 0 1
node 0 size: 2048 MB
node 1 cpus: 2 3
node 1 size: 2048 MB
node distances:
node   0   1
  0:  10  21
  1:  21  10
```

Key things to verify:
- **Number of nodes** matches your host (or `numa_mapping` configuration).
- **CPUs** are distributed across nodes (not all on node 0).
- **Memory** is split across nodes (not all on node 0).
- **Distances** mirror the host distances.

### 5.2 Check CPU-to-NUMA mapping

```bash
$ lscpu | grep -i numa
```

Expected:

```
NUMA node(s):          2
NUMA node0 CPU(s):     0,1
NUMA node1 CPU(s):     2,3
```

### 5.3 Check from /proc and /sys inside the guest

```bash
$ cat /sys/devices/system/node/node*/cpulist
```

Expected:

```
0-1
2-3
```

```bash
$ cat /sys/devices/system/node/node*/meminfo | grep MemTotal
```

Expected (values will vary based on your pod's memory request):

```
Node 0 MemTotal:     2097152 kB
Node 1 MemTotal:     2097152 kB
```

## Step 6: Verify NUMA on the Host

### 6.1 Check vCPU pinning

From the host, find the QEMU process and check its thread affinities:

```bash
$ QEMU_PID=$(pgrep -f "qemu.*numa-test")
$ ls /proc/${QEMU_PID}/task/ | while read tid; do
    echo "TID ${tid}: $(taskset -p ${tid} 2>/dev/null)"
  done
```

With NUMA pinning enabled, you should see vCPU threads pinned to specific
CPUs (not the full CPU mask). For example, on a 2-NUMA-node host with
CPUs 0-7 on node 0 and CPUs 8-15 on node 1:

```
TID 12345: pid 12345's current affinity mask: 1    # CPU 0
TID 12346: pid 12346's current affinity mask: 2    # CPU 1
TID 12347: pid 12347's current affinity mask: 100  # CPU 8
TID 12348: pid 12348's current affinity mask: 200  # CPU 9
```

### 6.2 Check the shim logs for NUMA configuration

```bash
$ POD_SANDBOX_ID=$(crictl pods --name numa-test -q)
$ journalctl -t kata | grep "${POD_SANDBOX_ID}" | grep -i numa
```

Look for lines like:

```
buildNUMATopology: creating 2 guest NUMA nodes
VFIO device NUMA placement validated  bdf=0000:41:00.0 host-numa=1 guest-numa=1
```

### 6.3 Check the QEMU command line

```bash
$ cat /proc/${QEMU_PID}/cmdline | tr '\0' '\n' | grep -E "numa|memory-backend"
```

Expected output (varies by configuration):

```
-object
memory-backend-ram,id=numa-mem0,size=2048M,host-nodes=0,policy=bind,share=on
-numa
node,nodeid=0,memdev=numa-mem0,cpus=0-1
-object
memory-backend-ram,id=numa-mem1,size=2048M,host-nodes=1,policy=bind,share=on
-numa
node,nodeid=1,memdev=numa-mem1,cpus=2-3
-numa
dist,src=0,dst=1,val=21
-numa
dist,src=1,dst=0,val=21
```

Key things to verify:
- Each `-object memory-backend-*` has `host-nodes=N` and `policy=bind`
  matching the correct host NUMA node.
- Each `-numa node` has a `cpus=` range and `memdev=` pointing to the
  correct memory backend.
- `-numa dist` entries mirror the host distances.

## Step 7: Verify GPU NUMA Placement (GPU Passthrough Only)

If using GPU passthrough, verify the device landed on the correct NUMA node:

### 7.1 Check host-side GPU NUMA node

```bash
$ GPU_BDF="0000:41:00.0"  # Replace with your GPU's PCI address
$ cat /sys/bus/pci/devices/${GPU_BDF}/numa_node
```

### 7.2 Check shim logs for VFIO placement validation

```bash
$ journalctl -t kata | grep -i "VFIO device NUMA"
```

Healthy output:

```
VFIO device NUMA placement validated  bdf=0000:41:00.0 host-numa=1 guest-numa=1
```

Warning output (indicates misconfiguration):

```
VFIO device on host NUMA node not covered by guest NUMA topology  bdf=0000:41:00.0 host-numa=2 covered-nodes=map[0:0 1:1]
```

If you see the warning, extend your `numa_mapping` to include the GPU's host
NUMA node.

### 7.3 Check GPU NUMA inside the guest

Inside the GPU pod, verify the GPU reports a valid NUMA node (not `-1`):

```bash
$ cat /sys/bus/pci/devices/*/numa_node
# Should show 0 or 1 (matching the host GPU's NUMA node), not -1.

$ nvidia-smi topo --matrix
# Shows the GPU's relationship to NUMA nodes from the guest perspective.
```

The runtime uses QEMU's `acpi-generic-initiator` object to wire each VFIO
device to the correct guest NUMA node.  If the guest reports `-1`, check
that the QEMU command line contains
`-object acpi-generic-initiator,id=gi-...,pci-dev=...,node=...`.

## How It Works

When a VM is created with NUMA enabled, the runtime:

1. **Discovers host NUMA**: Reads
   `/sys/devices/system/node/node*/distance` to build the host distance
   matrix.

2. **Right-sizes the topology** (auto-discovery only): When `numa_mapping`
   is empty, the runtime compares the sandbox's vCPU and memory budget
   against per-node host capacity (read from
   `/sys/devices/system/node/node*/meminfo` and `cpulist`). If any
   cold-plugged VFIO device pins the sandbox to specific host nodes, the
   chosen subset must cover those; otherwise the smallest single host
   node that fits the workload is picked. When the resulting subset has
   one node, the topology collapses to a flat (no `-numa`) layout so QEMU
   uses a single memory backend. Sandboxes that exceed any single node
   keep the full auto-derived multi-node topology. An explicit
   `numa_mapping` opts out of this step entirely and is honored verbatim.

3. **Builds guest topology**: Creates guest NUMA nodes with per-node memory
   backends (`policy=bind` to lock memory to host NUMA nodes), distributes
   vCPUs proportionally to host CPU counts, and mirrors distances. For
   confidential guests (SEV-SNP, TDX), QEMU automatically enables
   `guest_memfd` on each memory backend for private/shared memory
   attribute tracking (requires the cross-region conversion patch).

4. **Restructures SMP**: Sets `sockets = num_NUMA_nodes` and
   `cores = ceil(maxvcpus / num_NUMA_nodes)` so QEMU groups vCPUs by socket
   per NUMA node.

5. **Pins vCPUs** (when enabled): Each vCPU thread is pinned to a host CPU
   belonging to the same NUMA node. Right-sized single-node sandboxes
   also go through this NUMA-aware path, so all vCPUs land on the chosen
   host NUMA node's CPUs.

6. **Places VFIO devices on correct guest NUMA node**: For each
   cold-plugged VFIO device (e.g. GPU), the runtime looks up its host
   NUMA node, maps it to the corresponding guest NUMA node, and emits a
   QEMU `acpi-generic-initiator` object so the guest kernel reports the
   correct `numa_node` for the device.  This ensures GPU memory accesses
   stay NUMA-local.  If a device's host NUMA node is not covered by the
   guest topology, a warning is logged.

7. **Translates cpuset.mems**: Converts host NUMA node IDs to guest node IDs
   before forwarding to the agent.

## Troubleshooting

### Guest reports a single NUMA node on a multi-NUMA host

**Symptom:** Inside a small pod on a 2+ NUMA-node host, `numactl --hardware`
shows only one NUMA node, and the QEMU command line has no `-numa`
arguments.

**Cause:** Right-sizing collapsed the auto-derived topology because the
sandbox's vCPU + memory budget fits on one host NUMA node. This is the
intended optimization — the pod gets full memory locality without paying
the cross-node penalty for a workload that does not need it.

**Fix (only if you really want the multi-node layout):** either
- set an explicit `numa_mapping = ["0", "1"]` (or similar) — explicit
  mappings skip right-sizing and are honored verbatim, or
- raise the pod's `limits.cpu` / `limits.memory` so the sandbox truly
  exceeds any single host node's capacity.

### Multi-NUMA topology is skipped (too few vCPUs)

**Symptom:** The shim logs show:

```
DefaultMaxVCPUs < NUMA node count; skipping multi-NUMA topology  vcpus=1 numa-nodes=2
```

**Cause:** The pod requested fewer CPUs than there are NUMA nodes. Each
NUMA node needs at least one vCPU.

**Fix:** Request at least as many CPUs as NUMA nodes in the pod spec:

```yaml
resources:
  limits:
    cpu: "2"   # At least 2 for a 2-NUMA-node host
```

Or increase `default_vcpus` via a drop-in:

```bash
$ cat > /opt/kata/share/defaults/kata-containers/runtimes/qemu/config.d/50-default-vcpus.toml <<'EOF'
[hypervisor.qemu]
default_vcpus = 2
EOF
```

### vCPU pinning is skipped (empty CPUSet)

**Symptom:** The shim logs show:

```
sandbox CPUSet is empty; skipping vCPU pinning
```

**Cause:** The runtime could not determine a CPUSet for pinning. With
`cpuManagerPolicy: none` and multi-NUMA enabled, the runtime derives the
CPUSet from the guest NUMA nodes' `HostCPUs`. This message indicates no
NUMA topology was built (e.g., the host has only one NUMA node).

**Fix:** Verify:

1. The host has multiple NUMA nodes (`numactl --hardware`)
2. `enable_numa = true` is set in the Kata configuration
3. `enable_vcpus_pinning = true` is set in the Kata configuration
4. `static_sandbox_resource_mgmt = true` is set (so all vCPUs boot at start)

### NUMA pinning fallback warning

**Symptom:** The shim logs show:

```
NUMA node HostCPUs do not intersect sandbox CPUSet; falling back to full cpuset
```

**Cause:** The CPUs Kubernetes assigned to the pod do not overlap with the
host CPUs on the NUMA node. This means NUMA locality is lost for that node.

**Fix:** Verify that your `numa_mapping` matches the actual host topology:

```bash
$ numactl --hardware  # Check which CPUs are on which nodes
```

Ensure the Kubernetes node has CPUs from all mapped NUMA nodes available
for scheduling.

### Configuration validation error at startup

**Symptom:**

```
NUMA support requires static_sandbox_resource_mgmt to be enabled
```

**Fix:** Add `static_sandbox_resource_mgmt` via a drop-in:

```bash
$ cat > /opt/kata/share/defaults/kata-containers/runtimes/qemu/config.d/50-static-resources.toml <<'EOF'
[runtime]
static_sandbox_resource_mgmt = true
EOF
```

## Configuration Reference

| Option | Section | Default | Description |
|--------|---------|---------|-------------|
| `enable_numa` | `[hypervisor.qemu]` | `false` | Enable guest NUMA topology |
| `numa_mapping` | `[hypervisor.qemu]` | `[]` | Map guest NUMA nodes to host nodes. Empty = auto-discover with right-sizing (small sandboxes collapse to one node); non-empty = honored verbatim |
| `static_sandbox_resource_mgmt` | `[runtime]` | varies | Size VM at boot (required for NUMA) |
| `enable_vcpus_pinning` | `[runtime]` | `false` | Pin vCPU threads to host CPUs (NUMA-aware when NUMA enabled) |

## Limitations

- NUMA is only supported with the **Go runtime** and **QEMU** hypervisor.
- Only **amd64** and **arm64** architectures are supported.
- NUMA requires `static_sandbox_resource_mgmt = true` (no dynamic
  CPU/memory hotplug).
- The VM needs at least as many vCPUs as NUMA nodes. If fewer vCPUs are
  available, multi-NUMA is silently skipped.
- vCPU pinning with NUMA works best with `cpuManagerPolicy: none` (the
  default). Using `static` may restrict the pod's CPUSet to a single NUMA
  node, preventing balanced pinning across nodes.
- Confidential guests (SEV-SNP, TDX) with NUMA require a QEMU patch
  ([accel/kvm: Fix kvm_convert_memory calls crossing memory regions](https://github.com/AMDESE/qemu/commit/6b0eaa20))
  to handle page conversions that span multiple NUMA memory backends.
  The GPU-experimental QEMU builds (`gpu-snp`, `gpu-tdx`) include this
  patch. Without it, QEMU crashes with
  `ram_block_attributes_state_change, invalid range`.

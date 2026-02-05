# Kata Containers Lifecycle Management

## Overview

Kata Containers lifecycle management in Kubernetes consists of two operations:

1. **Installation** - Deploy Kata Containers to cluster nodes
2. **Upgrades** - Update Kata Containers to newer versions without disrupting workloads

The Kata Containers project provides two Helm charts to address these needs:

| Chart | Purpose |
|-------|---------|
| `kata-deploy` | Initial installation and configuration |
| `kata-lifecycle-manager` | Orchestrated rolling upgrades with verification |

---

## Installation with kata-deploy

The `kata-deploy` Helm chart installs Kata Containers across all (or selected) nodes using a Kubernetes DaemonSet. When deployed, it:

- Installs Kata runtime binaries on each node
- Configures the container runtime (containerd) to use Kata
- Registers RuntimeClasses (`kata-qemu-nvidia-gpu-snp`, `kata-qemu-nvidia-gpu-tdx`, `kata-qemu-nvidia-gpu`, etc.)

After installation, workloads can use Kata isolation by specifying `runtimeClassName: kata-qemu-nvidia-gpu-snp` (or another Kata RuntimeClass) in their pod spec.

---

## Upgrades with kata-lifecycle-manager

### The Problem

Standard `helm upgrade kata-deploy` updates all nodes simultaneously via the DaemonSet. This approach:

- Provides no per-node verification
- Offers no controlled rollback mechanism
- Can leave the cluster in an inconsistent state if something fails

### The Solution

The `kata-lifecycle-manager` Helm chart uses Argo Workflows to orchestrate upgrades with the following guarantees:

| Guarantee | Description |
|-----------|-------------|
| **Sequential Processing** | Nodes are upgraded one at a time |
| **Per-Node Verification** | A user-provided pod validates Kata functionality after each node upgrade |
| **Fail-Fast** | If verification fails, the workflow stops immediately |
| **Automatic Rollback** | On failure, Helm rollback is executed and the node is restored |

### Upgrade Flow

For each node in the cluster:

1. **Cordon** - Mark node as unschedulable
2. **Drain** (optional) - Evict existing workloads
3. **Upgrade** - Run `helm upgrade kata-deploy` targeting this node
4. **Wait** - Ensure kata-deploy DaemonSet pod is ready
5. **Verify** - Run verification pod to confirm Kata works
6. **Uncordon** - Mark node as schedulable again

If verification fails on any node, the workflow:
- Rolls back the Helm release
- Uncordons the node
- Stops processing (remaining nodes are not upgraded)

### Verification Pod

Users must provide a verification pod that tests Kata functionality. This pod:

- Uses a Kata RuntimeClass
- Is scheduled on the specific node being verified
- Runs whatever validation logic the user requires (smoke tests, attestation checks, etc.)

**Basic GPU Verification Example:**

For clusters with NVIDIA GPUs, the CUDA VectorAdd sample provides a more comprehensive verification:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: ${TEST_POD}
spec:
  runtimeClassName: kata-qemu-nvidia-gpu-snp # or kata-qemu-nvidia-gpu-tdx
  restartPolicy: Never
  nodeSelector:
    kubernetes.io/hostname: ${NODE}
  containers:
  - name: cuda-vectoradd
    image: nvcr.io/nvidia/k8s/cuda-sample:vectoradd-cuda12.5.0-ubuntu22.04
    resources:
      limits:
        nvidia.com/pgpu: "1"
        memory: 16Gi
```

This verifies that GPU passthrough works correctly with the upgraded Kata runtime.

The placeholders `${NODE}` and `${TEST_POD}` are substituted at runtime.

---

## Demo Recordings

| Demo | Description | Link |
|------|-------------|------|
| Sunny Path | Successful upgrade from 3.24.0 to 3.25.0 | [TODO] |
| Rainy Path | Failed verification triggers rollback | [TODO] |

---

## References

- [kata-deploy Helm Chart](tools/packaging/kata-deploy/helm-chart/README.md)
- [kata-lifecycle-manager Helm Chart](tools/packaging/kata-deploy/helm-chart/kata-lifecycle-manager/README.md)
- [kata-lifecycle-manager Design Document](docs/design/kata-lifecycle-manager-design.md)

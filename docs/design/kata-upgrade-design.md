# Kata Containers Upgrade Orchestration Design

## Summary

This document proposes a Helm chart-based orchestration solution for Kata Containers that
enables controlled, node-by-node upgrades with verification and rollback capabilities
using Argo Workflows.

## Motivation

### Problem Statement

Upgrading Kata Containers in a production Kubernetes cluster presents several challenges:

1. **Workload Scheduling Control**: New Kata workloads should not be scheduled on a node
   during upgrade until the new runtime is verified.

2. **Verification Gap**: There is no standardized way to verify that Kata is working correctly
   after an upgrade before allowing workloads to return to the node. This solution addresses
   the gap by running a user-provided verification pod on each upgraded node.

3. **Rollback Complexity**: If an upgrade fails, administrators must manually coordinate
   rollback across multiple nodes.

4. **Controlled Rollout**: Operators need the ability to upgrade nodes incrementally
   (canary approach) with fail-fast behavior if any node fails verification.

5. **Multi-Architecture Support**: The upgrade tooling must work across all architectures
   supported by Kata Containers (amd64, arm64, s390x, ppc64le).

### Current State

The `kata-deploy` Helm chart provides installation and configuration of Kata Containers,
including a post-install verification job. However, there is no built-in mechanism for
orchestrating upgrades across nodes in a controlled manner.

## Goals

1. Provide a standardized, automated way to upgrade Kata Containers node-by-node
2. Ensure each node is verified before returning to service
3. Support user-defined verification logic
4. Automatically rollback if verification fails
5. Work with the existing `kata-deploy` Helm chart
6. Support all Kata-supported architectures

## Non-Goals

1. Initial Kata Containers installation (use kata-deploy Helm chart for that)
2. Managing Kubernetes cluster upgrades
3. Providing Kata-specific verification logic (this is user responsibility)
4. Managing Argo Workflows installation

## Argo Workflows Dependency

### What Works Without Argo

The following components work independently of Argo Workflows:

| Component | Description |
|-----------|-------------|
| **kata-deploy Helm chart** | Full installation, configuration, `RuntimeClasses` |
| **Post-install verification** | Helm hook runs verification pod after install |
| **Label-gated deployment** | Progressive rollout via node labels |
| **Manual upgrades** | User can script: cordon, helm upgrade, verify, `uncordon` |

Users who do not want Argo can still:
- Install and configure Kata via kata-deploy
- Perform upgrades manually or with custom scripts
- Use the verification pod pattern in their own automation

### What Requires Argo

The kata-upgrade Helm chart provides orchestration via Argo Workflows:

| Feature | Description |
|---------|-------------|
| **Automated node-by-node upgrades** | Sequential processing with fail-fast |
| **Taint-based node selection** | Select nodes by taint key/value |
| **`WorkflowTemplate`** | Reusable upgrade workflow |
| **Rollback entrypoint** | `argo submit --entrypoint rollback-node` |
| **Status tracking** | Node annotations updated at each phase |

### For Users Already Using Argo

If your cluster already has Argo Workflows installed:

```bash
# Install kata-upgrade - integrates with your existing Argo installation
helm install kata-upgrade oci://ghcr.io/kata-containers/kata-deploy-charts/kata-upgrade \
  --set argoNamespace=argo \
  --set-file defaults.verificationPod=./verification-pod.yaml

# Trigger upgrades via argo CLI or integrate with existing workflows
argo submit -n argo --from workflowtemplate/kata-upgrade -p target-version=3.25.0
```

kata-upgrade can also be triggered by other Argo workflows, CI/CD pipelines, or `GitOps`
tools that support Argo.

### For Users Not Wanting Argo

If you prefer not to use Argo Workflows:

1. **Use kata-deploy directly** - handles installation and basic verification
2. **Script your own orchestration** - example approach:

```bash
#!/bin/bash
# Manual upgrade script (no Argo required)
set -euo pipefail

VERSION="3.25.0"

# Upgrade each node with Kata runtime
kubectl get nodes -l katacontainers.io/kata-runtime=true -o name | while read -r node_path; do
  NODE="${node_path#node/}"
  echo "Upgrading $NODE..."
  kubectl cordon "$NODE"
  
  helm upgrade kata-deploy oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy \
    --namespace kube-system \
    --version "$VERSION" \
    --reuse-values \
    --wait
  
  # Wait for DaemonSet pod on this node
  kubectl rollout status daemonset/kata-deploy -n kube-system
  
  # Run verification (apply your pod, wait, check exit code)
  kubectl apply -f verification-pod.yaml
  kubectl wait pod/kata-verify --for=jsonpath='{.status.phase}'=Succeeded --timeout=180s
  kubectl delete pod/kata-verify
  
  kubectl uncordon "$NODE"
  echo "$NODE upgraded successfully"
done
```

This approach requires more manual effort but avoids the Argo dependency.

## Proposed Design

### Architecture Overview

```text
┌─────────────────────────────────────────────────────────────────┐
│                    Argo Workflows Controller                    │
│                         (pre-installed)                         │
└────────────────────────────┬────────────────────────────────────┘
                             │
                             ▼
┌──────────────────────────────────────────────────────────────┐
│                    kata-upgrade Helm Chart                   │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                   WorkflowTemplate                     │  │
│  │  - upgrade-all-nodes (entrypoint)                      │  │
│  │  - upgrade-single-node (per-node steps)                │  │
│  │  - rollback-node (manual recovery)                     │  │
│  └────────────────────────────────────────────────────────┘  │
│  ┌────────────────────────────────────────────────────────┐  │
│  │                   RBAC Resources                       │  │
│  │  - ServiceAccount                                      │  │
│  │  - ClusterRole (node, pod, helm operations)            │  │
│  │  - ClusterRoleBinding                                  │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                    kata-deploy Helm Chart                       │
│                   (existing installation)                       │
└─────────────────────────────────────────────────────────────────┘
```

### Upgrade Flow

For each node selected by the upgrade label:

```text
┌────────────┐    ┌──────────────┐    ┌────────────┐    ┌────────────┐
│  Prepare   │───▶│  Cordon      │───▶│  Upgrade   │───▶│Wait Ready  │
│ (annotate) │    │  (mark       │    │  (helm     │    │(kata-deploy│
│            │    │unschedulable)│    │ upgrade)   │    │ DaemonSet) │
└────────────┘    └──────────────┘    └────────────┘    └────────────┘
                                                               │
                                                               ▼
                  ┌────────────┐    ┌──────────────┐    ┌────────────┐
                  │  Complete  │◀───│   Uncordon   │◀───│  Verify    │
                  │ (annotate  │    │  (mark       │    │  (user pod)│
                  │  version)  │    │schedulable)  │    │            │
                  └────────────┘    └──────────────┘    └────────────┘
```

**Note:** Drain is not required for Kata upgrades. Running Kata VMs continue using
the in-memory binaries. Only new workloads use the upgraded binaries. Cordon ensures
the verification pod runs before any new workloads are scheduled with the new runtime.

**Optional Drain:** For users who prefer to evict workloads before any maintenance
operation, an optional drain step can be enabled via `drain-enabled=true`. When
enabled, an additional drain step runs after cordon and before upgrade.

### Node Selection Model

Nodes can be selected for upgrade using **labels**, **taints**, or **both**.

**Label-based selection:**

```bash
# Select nodes by label
argo submit -n argo --from workflowtemplate/kata-upgrade \
  -p target-version=3.25.0 \
  -p node-selector="katacontainers.io/kata-upgrade-window=true"
```

**Taint-based selection:**

Some organizations use taints to mark nodes for maintenance. The workflow supports
selecting nodes by taint key and optionally taint value:

```bash
# Select nodes with a specific taint
kubectl taint nodes worker-1 kata-upgrade=pending:NoSchedule

argo submit -n argo --from workflowtemplate/kata-upgrade \
  -p target-version=3.25.0 \
  -p node-taint-key=kata-upgrade \
  -p node-taint-value=pending
```

**Combined selection:**

Labels and taints can be used together for precise targeting:

```bash
argo submit -n argo --from workflowtemplate/kata-upgrade \
  -p target-version=3.25.0 \
  -p node-selector="node-pool=kata-pool" \
  -p node-taint-key=maintenance
```

This allows operators to:
1. Upgrade a single canary node first
2. Gradually add nodes to the upgrade window
3. Control upgrade timing via `GitOps` or automation
4. Integrate with existing taint-based maintenance workflows

### Node Pool Support

The node selector and taint selector parameters enable basic node pool targeting:

```bash
# Upgrade only nodes matching a specific node pool label
argo submit -n argo --from workflowtemplate/kata-upgrade \
  -p target-version=3.25.0 \
  -p node-selector="node-pool=kata-pool"
```

**Current Capabilities:**

| Feature | Status | Chart | Notes |
|---------|--------|-------|-------|
| Label-based selection | Supported | kata-upgrade | Works with any label combination |
| Taint-based selection | Supported | kata-upgrade | Select by taint key/value |
| Sequential upgrades | Supported | kata-upgrade | One node at a time with fail-fast |
| Pool-specific verification pods | Not supported | kata-upgrade | Same verification for all nodes |
| Pool-ordered upgrades | Not supported | kata-upgrade | Upgrade pool A before pool B |

See the [Potential Enhancements](#potential-enhancements) section for future work.

### Verification Model

**Verification runs on each node that is upgraded.** The node is only `uncordoned` after
its verification pod succeeds. If verification fails, automatic rollback is triggered
to restore the previous version before `uncordoning` the node.

**Common failure modes detected by verification:**
- Pod stuck in Pending/`ContainerCreating` (runtime can't start VM)
- Pod crashes immediately (containerd/CRI-O configuration issues)
- Pod times out (resource issues, image pull failures)
- Pod exits with non-zero code (verification logic failed)

All of these trigger automatic rollback. The workflow logs include pod status, events,
and logs to help diagnose the issue.

The user provides a complete Pod YAML that:
- Uses the Kata runtime class they want to verify
- Contains their verification logic (e.g., attestation checks)
- Exits 0 on success, non-zero on failure
- Includes tolerations for cordoned nodes (verification runs while node is cordoned)
- Includes a `nodeSelector` to ensure it runs on the specific node being upgraded

When upgrading multiple nodes (via label selector), nodes are processed sequentially.
For each node, the following placeholders are substituted with that node's specific values,
ensuring the verification pod runs on the exact node that was just upgraded:

- `${NODE}` - The hostname of the node being upgraded/verified
- `${TEST_POD}` - A generated unique pod name

Example verification pod:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: ${TEST_POD}
spec:
  runtimeClassName: kata-qemu
  restartPolicy: Never
  nodeSelector:
    kubernetes.io/hostname: ${NODE}
  tolerations:
    - operator: Exists    # Required: node is cordoned during verification
  containers:
    - name: verify
      image: quay.io/kata-containers/alpine-bash-curl:latest
      command: ["uname", "-a"]
```

This design keeps verification logic entirely in the user's domain, supporting:
- Different runtime classes (`kata-qemu`, `kata-qemu-snp`, `kata-qemu-tdx`, etc.)
- TEE-specific attestation verification
- GPU/accelerator validation
- Custom application smoke tests

### Sequential Execution with Fail-Fast

Nodes are upgraded strictly sequentially using recursive Argo templates. This design
ensures that if any node fails verification, the workflow stops immediately before
touching remaining nodes, preventing a mixed-version fleet.

Alternative approaches considered:
- **`withParam` + semaphore**: Provides cleaner UI but semaphore only controls concurrency,
  not failure propagation. Other nodes would still proceed after one fails.
- **`withParam` + `failFast`**: Would be ideal, but Argo only supports `failFast` for DAG
  tasks, not for steps with `withParam`.

The recursive template approach (`upgrade-node-chain`) naturally provides fail-fast
behavior because if any step in the chain fails, the recursion stops.

### Status Tracking

Node upgrade status is tracked via Kubernetes annotations:

| Annotation | Values |
|------------|--------|
| `katacontainers.io/kata-upgrade-status` | preparing, cordoned, draining, upgrading, verifying, completed, rolling-back, rolled-back |
| `katacontainers.io/kata-current-version` | Version string (e.g., "3.25.0") |

This enables:
- Monitoring upgrade progress via `kubectl get nodes`
- Integration with external monitoring systems
- Recovery from interrupted upgrades

### Rollback Support

**Automatic rollback on verification failure:** If the verification pod fails (non-zero exit),
kata-upgrade automatically:
1. Runs `helm rollback` to revert to the previous Helm release
2. Waits for kata-deploy DaemonSet to be ready with the previous version
3. `Uncordons` the node
4. Annotates the node with `rolled-back` status

This ensures nodes are never left in a broken state.

**Manual rollback:** For cases where you need to rollback a successfully upgraded node:

```bash
argo submit -n argo --from workflowtemplate/kata-upgrade \
  --entrypoint rollback-node \
  -p node-name=worker-1
```

## Components

### Container Images

Two multi-architecture container images are built and published:

| Image | Purpose | Architectures |
|-------|---------|---------------|
| `quay.io/kata-containers/kubectl:latest` | Kubernetes operations | amd64, arm64, s390x, ppc64le |
| `quay.io/kata-containers/helm:latest` | Helm operations | amd64, arm64, s390x, ppc64le |

Images are rebuilt weekly to pick up security updates and tool version upgrades.

### Helm Chart Structure

```text
kata-upgrade/
├── Chart.yaml                  # Chart metadata
├── values.yaml                 # Configurable defaults
├── README.md                   # Usage documentation
└── templates/
    ├── _helpers.tpl            # Template helpers
    ├── rbac.yaml               # ServiceAccount, ClusterRole, ClusterRoleBinding
    └── workflow-template.yaml  # Argo `WorkflowTemplate`
```

### RBAC Requirements

The workflow requires the following permissions:

| Resource | Verbs | Purpose |
|----------|-------|---------|
| nodes | get, list, watch, patch | `cordon`/`uncordon`, annotations |
| pods | get, list, watch, create, delete | Verification pods |
| pods/log | get | Verification output |
| `daemonsets` | get, list, watch | Wait for `kata-deploy` |

## User Experience

### Installation

```bash
# Install kata-upgrade with verification config
helm install kata-upgrade oci://ghcr.io/kata-containers/kata-deploy-charts/kata-upgrade \
  --set-file defaults.verificationPod=/path/to/verification-pod.yaml
```

### Triggering an Upgrade

```bash
# Label nodes for upgrade
kubectl label node worker-1 katacontainers.io/kata-upgrade-window=true

# Submit upgrade workflow
argo submit -n argo --from workflowtemplate/kata-upgrade \
  -p target-version=3.25.0

# Watch progress
argo watch @latest
```

### Monitoring

```bash
kubectl get nodes \
  -L katacontainers.io/kata-runtime \
  -L katacontainers.io/kata-upgrade-status \
  -L katacontainers.io/kata-current-version
```

## Security Considerations

1. **Namespace-Scoped Templates**: The chart creates a `WorkflowTemplate` (namespace-scoped)
   rather than `ClusterWorkflowTemplate` by default, reducing blast radius.

2. **Required Verification**: The chart fails to install if `defaults.verificationPod` is
   not provided, ensuring upgrades are always verified.

3. **Minimal RBAC**: The `ServiceAccount` has only the permissions required for upgrade
   operations.

4. **User-Controlled Verification**: Verification logic is entirely user-defined, avoiding
   any hardcoded assumptions about what "working" means.

## Integration with Release Process

The `kata-upgrade` chart is:
- Packaged alongside `kata-deploy` during releases
- Published to the same OCI registries (`quay.io`, `ghcr.io`)
- Versioned to match `kata-deploy`

## Potential Enhancements

The following enhancements could be considered if needed:

### kata-upgrade

1. **Pool-Specific Verification**: Different verification pods for different node pools
   (e.g., GPU nodes vs. CPU-only nodes).

2. **Ordered Pool Upgrades**: Upgrade node pool A completely before starting pool B.

## Alternatives Considered

### 1. DaemonSet-Based Upgrades

Using a DaemonSet to coordinate upgrades on each node.

**Rejected because**: DaemonSets don't provide the node-by-node sequencing and
verification workflow needed for controlled upgrades.

### 2. Operator Pattern

Building a Kubernetes Operator to manage upgrades.

**Rejected because**: Adds significant complexity and maintenance burden. Argo Workflows
is already widely adopted and provides the orchestration primitives needed.

### 3. Shell Script Orchestration

Providing a shell script that loops through nodes.

**Rejected because**: Less reliable, harder to monitor, no built-in retry/recovery,
and doesn't integrate with Kubernetes-native tooling.

## References

- [kata-deploy Helm Chart](https://github.com/kata-containers/kata-containers/tree/main/tools/packaging/kata-deploy/helm-chart/kata-deploy)
- [Argo Workflows](https://argoproj.github.io/argo-workflows/)
- [Helm Documentation](https://helm.sh/docs/)

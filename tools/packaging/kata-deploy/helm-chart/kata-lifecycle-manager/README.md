# Kata Lifecycle Manager Helm Chart

Argo Workflows-based lifecycle management for Kata Containers.

This chart installs a namespace-scoped `WorkflowTemplate` that performs controlled,
node-by-node upgrades of kata-deploy with verification and automatic rollback on failure.

## Prerequisites

- Kubernetes cluster with kata-deploy installed via Helm
- [Argo Workflows](https://argoproj.github.io/argo-workflows/) v3.4+ installed
- `helm` and `argo` CLI tools
- **Verification pod spec** (see [Verification Pod](#verification-pod-required))

## Installation

```bash
# From OCI registry (when published)
helm install kata-lifecycle-manager oci://ghcr.io/kata-containers/kata-deploy-charts/kata-lifecycle-manager

# From local source
helm install kata-lifecycle-manager ./kata-lifecycle-manager
```

## Verification Pod (Required)

A verification pod is **required** to validate each node after upgrade. The chart
will fail to install without one.

### Option A: Bake into kata-lifecycle-manager (recommended)

Provide the verification pod when installing the chart:

```bash
helm install kata-lifecycle-manager ./kata-lifecycle-manager \
  --set-file defaults.verificationPod=./my-verification-pod.yaml
```

This verification pod is baked into the `WorkflowTemplate` and used for all upgrades.

### Option B: Override at workflow submission

One-off override for a specific upgrade run. The pod spec must be base64-encoded
because Argo workflow parameters don't handle multi-line YAML reliably:

```bash
argo submit -n argo --from workflowtemplate/kata-lifecycle-manager \
  -p target-version=3.25.0 \
  -p verification-pod="$(base64 -w0 < ./my-verification-pod.yaml)"
```

**Note:** During helm upgrade, kata-`kata-deploy`'s own verification is disabled
(`--set verification.pod=""`). This is because kata-`kata-deploy`'s verification is
cluster-wide (designed for initial install), while kata-lifecycle-manager performs
per-node verification with proper placeholder substitution.

### Verification Pod Spec

Create a pod spec that validates your Kata deployment. The pod should exit 0 on success,
non-zero on failure.

**Example (`my-verification-pod.yaml`):**

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
    - operator: Exists
  containers:
    - name: verify
      image: quay.io/kata-containers/alpine-bash-curl:latest
      command:
        - sh
        - -c
        - |
          echo "=== Kata Verification ==="
          echo "Node: ${NODE}"
          echo "Kernel: $(uname -r)"
          echo "SUCCESS: Pod running with Kata runtime"
```

### Placeholders

| Placeholder | Description |
|-------------|-------------|
| `${NODE}` | Node hostname being upgraded/verified |
| `${TEST_POD}` | Generated unique pod name |

**You are responsible for:**
- Setting the `runtimeClassName` in your pod spec
- Defining the verification logic in your container
- Using the exit code to indicate success (0) or failure (non-zero)

**Failure modes detected:**
- Pod stuck in Pending/`ContainerCreating` (runtime can't start VM)
- Pod crashes immediately (containerd/CRI-O configuration issues)
- Pod times out (resource issues, image pull failures)
- Pod exits with non-zero code (verification logic failed)

All of these trigger automatic rollback.

## Usage

### 1. Select Nodes for Upgrade

Nodes can be selected using **labels**, **taints**, or **both**.

**Option A: Label-based selection (default)**

```bash
# Label nodes for upgrade
kubectl label node worker-1 katacontainers.io/kata-lifecycle-manager-window=true

# Trigger upgrade
argo submit -n argo --from workflowtemplate/kata-lifecycle-manager \
  -p target-version=3.25.0 \
  -p node-selector="katacontainers.io/kata-lifecycle-manager-window=true"
```

**Option B: Taint-based selection**

```bash
# Taint nodes for upgrade
kubectl taint nodes worker-1 kata-lifecycle-manager=pending:NoSchedule

# Trigger upgrade using taint selector
argo submit -n argo --from workflowtemplate/kata-lifecycle-manager \
  -p target-version=3.25.0 \
  -p node-taint-key=kata-lifecycle-manager \
  -p node-taint-value=pending
```

**Option C: Combined selection**

```bash
# Use both labels and taints for precise targeting
argo submit -n argo --from workflowtemplate/kata-lifecycle-manager \
  -p target-version=3.25.0 \
  -p node-selector="node-pool=kata-pool" \
  -p node-taint-key=kata-lifecycle-manager
```

### 2. Trigger Upgrade

```bash
argo submit -n argo --from workflowtemplate/kata-lifecycle-manager \
  -p target-version=3.25.0

# Watch progress
argo watch @latest
```

### 3. Sequential Upgrade Behavior

Nodes are upgraded **sequentially** (one at a time) to ensure fleet consistency.
If any node fails verification, the workflow stops immediately and that node is
rolled back. This prevents ending up with a mixed fleet where some nodes have
the new version and others have the old version.

## Configuration

| Parameter | Description | Default |
|-----------|-------------|---------|
| `argoNamespace` | Namespace for Argo resources | `argo` |
| `defaults.helmRelease` | kata-deploy Helm release name | `kata-deploy` |
| `defaults.helmNamespace` | kata-deploy namespace | `kube-system` |
| `defaults.nodeSelector` | Node label selector (optional if using taints) | `""` |
| `defaults.nodeTaintKey` | Taint key for node selection | `""` |
| `defaults.nodeTaintValue` | Taint value filter (optional) | `""` |
| `defaults.verificationNamespace` | Namespace for verification pods | `default` |
| `defaults.verificationPod` | Pod YAML for verification **(required)** | `""` |
| `defaults.drainEnabled` | Enable node drain before upgrade | `false` |
| `defaults.drainTimeout` | Timeout for drain operation | `300s` |
| `images.helm` | Helm container image (multi-arch) | `quay.io/kata-containers/helm:latest` |
| `images.kubectl` | `kubectl` container image (multi-arch) | `quay.io/kata-containers/kubectl:latest` |

## Workflow Parameters

When submitting a workflow, you can override:

| Parameter | Description |
|-----------|-------------|
| `target-version` | **Required** - Target Kata version |
| `helm-release` | Helm release name |
| `helm-namespace` | Namespace of kata-deploy |
| `node-selector` | Label selector for nodes |
| `node-taint-key` | Taint key for node selection |
| `node-taint-value` | Taint value filter |
| `verification-namespace` | Namespace for verification pods |
| `verification-pod` | Pod YAML with placeholders |
| `drain-enabled` | Whether to drain nodes before upgrade |
| `drain-timeout` | Timeout for drain operation |

## Upgrade Flow

For each node selected by the node-selector label:

1. **Prepare**: Annotate node with upgrade status
2. **Cordon**: Mark node as `unschedulable`
3. **Drain** (optional): Evict pods if `drain-enabled=true`
4. **Upgrade**: Run `helm upgrade` for kata-deploy
5. **Wait**: Wait for kata-deploy DaemonSet pod to be ready
6. **Verify**: Run verification pod and check exit code
7. **On Success**: `Uncordon` node, proceed to next node
8. **On Failure**: Automatic rollback, `uncordon`, workflow stops

Nodes are upgraded **sequentially** (one at a time). If verification fails on any node,
the workflow stops immediately, preventing a mixed-version fleet.

### When to Use Drain

**Default (drain disabled):** Drain is not required for Kata upgrades. Running Kata
VMs continue using the in-memory binaries. Only new workloads use the upgraded
binaries.

**Optional drain:** Enable drain if you prefer to evict all workloads before any
maintenance operation, or if your organization's operational policies require it:

```bash
# Enable drain when installing the chart
helm install kata-lifecycle-manager ./kata-lifecycle-manager \
  --set defaults.drainEnabled=true \
  --set defaults.drainTimeout=600s \
  --set-file defaults.verificationPod=./my-verification-pod.yaml

# Or override at workflow submission time
argo submit -n argo --from workflowtemplate/kata-lifecycle-manager \
  -p target-version=3.25.0 \
  -p drain-enabled=true \
  -p drain-timeout=600s
```

## Rollback

**Automatic rollback on verification failure:** If the verification pod fails (non-zero exit),
kata-lifecycle-manager automatically:
1. Runs `helm rollback` to revert to the previous Helm release
2. Waits for kata-deploy DaemonSet to be ready with the previous version
3. `Uncordons` the node
4. Annotates the node with `rolled-back` status

This ensures nodes are never left in a broken state.

**Manual rollback:** For cases where you need to rollback a successfully upgraded node:

```bash
argo submit -n argo --from workflowtemplate/kata-lifecycle-manager \
  --entrypoint rollback-node \
  -p node-name=worker-1
```

## Monitoring

Check node annotations to monitor upgrade progress:

```bash
kubectl get nodes \
  -L katacontainers.io/kata-lifecycle-manager-status \
  -L katacontainers.io/kata-current-version
```

| Annotation | Description |
|------------|-------------|
| `katacontainers.io/kata-lifecycle-manager-status` | Current upgrade phase |
| `katacontainers.io/kata-current-version` | Version after successful upgrade |

Status values:
- `preparing` - Upgrade starting
- `cordoned` - Node marked `unschedulable`
- `draining` - Draining pods (only if drain-enabled=true)
- `upgrading` - Helm upgrade in progress
- `verifying` - Verification pod running
- `completed` - Upgrade successful
- `rolling-back` - Rollback in progress (automatic on verification failure)
- `rolled-back` - Rollback completed

## Known Limitations

### Cluster-Wide DaemonSet Updates

The kata-deploy Helm chart uses a DaemonSet, which means `helm upgrade` updates
all nodes simultaneously at the Kubernetes level, even though this workflow
processes nodes sequentially for verification.

**Current behavior:**

1. Node A is cordoned and upgraded
2. Node A verification passes, Node A is `uncordoned`
3. New workloads can now start on Node A using the **new** Kata version
4. Node B verification fails
5. Automatic rollback reverts kata-deploy cluster-wide to the **old** version
6. Workloads that started on Node A between steps 2-5 continue running with
   the new version's in-memory binaries, while new workloads use the old version

This is generally acceptable because:
- Running Kata VMs continue functioning (they use in-memory binaries)
- New workloads use the rolled-back version
- The cluster reaches a consistent state for new workloads

**Future improvement:** A two-phase approach could cordon all target nodes upfront,
perform the upgrade, verify all nodes, and only `uncordon` after all verifications
pass. This would prevent any new workloads from using the new version until the
entire upgrade is validated, at the cost of longer node unavailability.

## For Projects Using kata-deploy

Any project that uses the kata-deploy Helm chart can install this companion chart
to get upgrade orchestration:

```bash
# Install kata-deploy
helm install kata-deploy oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy \
  --namespace kube-system

# Install upgrade tooling with your verification config
helm install kata-lifecycle-manager oci://ghcr.io/kata-containers/kata-deploy-charts/kata-lifecycle-manager \
  --set-file defaults.verificationPod=./my-verification-pod.yaml

# Trigger upgrade
argo submit -n argo --from workflowtemplate/kata-lifecycle-manager \
  -p target-version=3.25.0
```

## License

Apache License 2.0

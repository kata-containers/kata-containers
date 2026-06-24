# Helm Configuration

## Parameters

The helm chart provides a comprehensive set of configuration options. You may view the parameters and their descriptions by going to the [GitHub source](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/values.yaml) or by using helm:

```sh
# List available kata-deploy chart versions:
#   helm search repo kata-deploy-charts/kata-deploy --versions
#
# Then replace X.Y.Z below with the desired chart version:
helm show values --version X.Y.Z oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy
```

### shims

Kata ships with a number of pre-built artifacts and runtimes. You may selectively enable or disable specific shims. For example:

```yaml title="values.yaml"
shims:
  disableAll: true
  qemu:
    enabled: true
  qemu-nvidia-gpu:
    enabled: true
  qemu-nvidia-gpu-snp:
    enabled: false

```

Shims can also have configuration options specific to them:

```yaml
  qemu-nvidia-gpu:
    enabled: ~
    supportedArches:
      - amd64
    dropIn: |
      [agent.kata]
      dial_timeout = 999
    allowedHypervisorAnnotations: []
    containerd:
      snapshotter: ""
    runtimeClass:
      # This label is automatically added by gpu-operator. Override it
      # if you want to use a different label.
      # Uncomment once GPU Operator v26.3 is out
      # nodeSelector:
        # nvidia.com/cc.ready.state: "false"
```

The optional `shims.<shim>.dropIn` field lets you add a custom Kata drop-in for a
default (non-custom) runtime. kata-deploy writes it as
`config.d/50-user-overrides.toml` for that shim.

It's best to reference the default `values.yaml` file above for more details.

### Custom Runtimes

Kata allows you to create custom runtime configurations. This is done by overlaying one of the pre-existing runtime configs with user-provided configs. For example, we can use the `qemu-nvidia-gpu` as a base config and overlay our own parameters to it:

```yaml
customRuntimes:
  enabled: false
  runtimes:
    my-gpu-runtime:
      baseConfig: "qemu-nvidia-gpu"  # Required: existing config to use as base
      dropIn: |                      # Optional: overrides via config.d mechanism
        [hypervisor.qemu]
        default_memory = 1024
        default_vcpus = 4
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: kata-my-gpu-runtime
          labels:
            app.kubernetes.io/managed-by: kata-deploy
        handler: kata-my-gpu-runtime
        overhead:
          podFixed:
            memory: "640Mi"
            cpu: "500m"
        scheduling:
          nodeSelector:
            katacontainers.io/kata-runtime: "true"
      # Optional: CRI-specific configuration
      containerd:
        snapshotter: "nydus"  # Configure containerd snapshotter (nydus, erofs, etc.)
      crio:
        pullType: "guest-pull"  # Configure CRI-O runtime_pull_image = true
```

Again, view the default [`values.yaml`](#parameters) file for more details.

### Drop-In Runtime Configuration

The base runtime configuration shipped with Kata Containers can be modified using an
overlay method. This can be done directly on the filesystem using the instructions
found [here](runtime-configuration.md#drop-in-files).
You can also use the `customRuntimes.runtimes.[name].dropIn` configuration in the helm
chart to achieve the same results.

## Deployment Modes (DaemonSet vs Job)

The chart can install Kata on nodes in one of two ways, selected with the
top-level `deploymentMode` value:

- **`daemonset`** (default): the long-running `kata-deploy` DaemonSet installs
  Kata on every matching node and reverts it when the pod is terminated (i.e. on
  uninstall). This is the historical behavior and is unchanged.
- **`job`**: there is **no always-on component**. A tiny *dispatcher* Job (the
  dispatcher, `kata-deploy-job-dispatcher`) runs as a `post-install`/`post-upgrade` hook,
  enumerates the selected nodes **live** via the Kubernetes API, and creates one
  node-pinned install `Job` per node. Each per-node Job runs the staged install
  pipeline as ordered `initContainers` and then exits:

  ```
  host-check -> artifacts -> cri   (initContainers)  ->  label (main)
  ```

  On `helm uninstall`, a `pre-delete` dispatcher fans out per-node Jobs that run
  the pipeline in reverse (`unlabel -> revert-cri -> remove-artifacts`). Unlike
  the DaemonSet, **nothing keeps running on the node after installation
  completes**, and the dispatcher itself only ever talks to the API server — it
  never touches the host (so it ships as a separate, minimal image,
  `job.dispatcherImage`).

  The privilege split is explicit: the dispatcher pod runs **fully unprivileged**
  (`runAsNonRoot`, all capabilities dropped, no privilege escalation, read-only
  root filesystem, `RuntimeDefault` seccomp) under a **dedicated minimal
  ServiceAccount** whose only rights are `nodes: list` (cluster-scoped) and
  managing Jobs in the release namespace. All privileged, host-mutating work
  stays in the per-node Jobs, which continue to use the `kata-deploy`
  ServiceAccount.

```yaml title="values.yaml"
deploymentMode: job
```

#### Why a dispatcher instead of Helm-rendered per-node Jobs

Rendering one Job per node directly in the chart does not scale: Helm stores the
whole rendered release in a single (~1 MiB) Secret and runs hook resources
sequentially, so large fleets blow the size limit and/or take far too long. A
single `Indexed Job` or a `JobSet` removes those limits but **cannot guarantee
one pod per node** once `parallelism < node-count`: Kubernetes' topology-spread
and affinity scheduling ignore *completed* pods, so as paced pods finish, later
pods pile onto a subset of nodes and leave others uncovered.

The dispatcher sidesteps both problems: the Helm release stays O(1) (just the
dispatcher + a constant-size ConfigMap holding the per-node Job templates), node
membership is resolved at run time, and the dispatcher itself paces the rollout
(at most `job.parallelism` per-node Jobs in flight) while **guaranteeing one Job
per node**. Per-node Jobs are garbage-collected via an `ownerReference` to the
dispatcher and `job.ttlSecondsAfterFinished`.

### Adding nodes in `job` mode

The dispatcher only runs on `helm install` / `helm upgrade` / `helm uninstall`.
There is **no dispatcher watching for new nodes**, so when you add nodes later,
re-run `helm upgrade`; the dispatcher re-enumerates the cluster and installs the
new nodes:

```sh
helm upgrade kata-deploy "${CHART}" --version "${VERSION}" --reuse-values
```

Each per-node stage is idempotent (it skips when already applied), so the
upgrade only does real work on the newly added nodes.

### Recovering from a failed or deleted dispatcher

The dispatcher runs as a **blocking** `post-install`/`post-upgrade` hook Job with
`restartPolicy: Never` and `backoffLimit: 0`, so if its pod is evicted, drained,
or deleted mid-rollout the Job is marked *failed* and is **not** restarted
automatically — `helm install`/`helm upgrade` surfaces the failure rather than
leaving you silently half-installed.

What survives the dispatcher dying:

- **Per-node Jobs already created keep running.** They are independent,
  `nodeName`-pinned Jobs, not children of the dispatcher pod, so installs that
  were already dispatched run to completion and those nodes get labeled. Only
  nodes still queued (never dispatched) are skipped, so at worst you get
  *partial coverage* — never a half-mutated host, because each stage is
  idempotent.
- Those per-node Jobs carry a (non-controller) `ownerReference` to the dispatcher
  Job, so they survive *pod* deletion but are garbage-collected once the
  dispatcher **Job** itself is removed or its `job.ttlSecondsAfterFinished`
  elapses. Keep that TTL comfortably larger than a single node's install so
  in-flight Jobs are not reaped early.

Recovery is the same one-liner as adding nodes — re-run `helm upgrade`:

```sh
helm upgrade kata-deploy "${CHART}" --version "${VERSION}" --reuse-values
```

The `before-hook-creation` delete policy first removes the stale dispatcher Job
(cascading away any leftover per-node Jobs); the fresh dispatcher then
re-enumerates nodes live, recreates the per-node Jobs (adopting any that still
exist rather than duplicating them), and because every stage is idempotent the
already-installed nodes are fast no-ops. Coverage converges on the re-run.

### Choosing which nodes get a Job

In `job` mode, node selection is configured under the `job` key, with the
following precedence (highest first):

1. `job.nodes`: an explicit list of node names, passed to the dispatcher verbatim.
2. `job.nodeSelector` (an equality map) **ANDed with**
   `job.nodeSelectorExpressions` (Kubernetes label-selector requirements using
   the operators `In`, `NotIn`, `Exists`, `DoesNotExist`). These are compiled
   into a single label-selector string that the dispatcher resolves live.
3. If both are empty, **all** nodes are targeted.

By **default the expressions target worker (non-control-plane) nodes**, so no
custom node labeling is required (this differs from the DaemonSet `nodeSelector`
examples above, which rely on you labeling nodes). Override as needed:

```yaml title="values.yaml"
# Target nodes carrying a specific label:
job:
  nodeSelector:
    kata-containers: "enabled"

# Target every node, including control-plane (e.g. single-node clusters / CI):
job:
  nodeSelectorExpressions: []

# Richer expressions:
job:
  nodeSelectorExpressions:
    - { key: kubernetes.io/os, operator: In, values: ["linux"] }
    - { key: node-role.kubernetes.io/control-plane, operator: DoesNotExist }

# Pin to explicit nodes:
job:
  nodes: ["worker-1", "worker-2"]
```

Use `job.parallelism` to pace the rollout — it caps how many per-node Jobs run
concurrently (e.g. to limit how many CRI runtimes restart at once on a big
fleet). It is effectively capped at the number of targeted nodes.

### Choosing which nodes are cleaned up on uninstall

Because the cleanup dispatcher resolves nodes **live when it runs** at
`helm uninstall` (the dispatcher does the lookup, not Helm at render time), the
node set is *not* frozen into the stored release. This means the **default
cleanup selector can simply be "nodes carrying the
`katacontainers.io/kata-runtime` label"** — i.e. exactly the nodes the install
actually labeled, regardless of how the install selector has drifted since.

Override it under `job.cleanup`, with the same precedence/semantics as install
(`cleanup.nodes`, then `cleanup.nodeSelector` ANDed with
`cleanup.nodeSelectorExpressions`, else all nodes):

```yaml title="values.yaml"
# Only uninstall from specific nodes:
job:
  cleanup:
    nodes: ["worker-1"]

# Use an explicit selector instead of the kata-runtime label default:
job:
  cleanup:
    nodeSelectorExpressions:
      - { key: node-role.kubernetes.io/control-plane, operator: DoesNotExist }
```

See the default [`values.yaml`](#parameters) for the remaining `job.*` options
(e.g. `dispatcherImage`, `parallelism`, `ttlSecondsAfterFinished`,
`backoffLimit`).

## Examples

We provide a few examples that you can pass to helm via the `-f`/`--values` flag.

### [`try-kata-tee.values.yaml`](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-tee.values.yaml)

This file enables only the TEE (Trusted Execution Environment) shims for confidential computing:

```sh
helm install kata-deploy oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy \
  --version VERSION \
  -f try-kata-tee.values.yaml
```

Includes:

- `qemu-snp` - AMD SEV-SNP (amd64)
- `qemu-tdx` - Intel TDX (amd64)
- `qemu-se` - IBM Secure Execution for Linux (SEL) (s390x)
- `qemu-se-runtime-rs` - IBM Secure Execution for Linux (SEL) Rust runtime (s390x)
- `qemu-coco-dev` - Confidential Containers development (amd64, s390x)
- `qemu-coco-dev-runtime-rs` - Confidential Containers development Rust runtime (amd64, arm64, s390x)

### [`try-kata-nvidia-gpu.values.yaml`](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-nvidia-gpu.values.yaml)

This file enables only the NVIDIA GPU-enabled shims and installs them using the
[`job` deployment mode](#deployment-modes-daemonset-vs-job) (no always-on
DaemonSet on the node):

```sh
helm install kata-deploy oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy \
  --version VERSION \
  -f try-kata-nvidia-gpu.values.yaml
```

Includes:

- `qemu-nvidia-gpu` - Standard NVIDIA GPU support (amd64)
- `qemu-nvidia-gpu-snp` - NVIDIA GPU with AMD SEV-SNP (amd64)
- `qemu-nvidia-gpu-tdx` - NVIDIA GPU with Intel TDX (amd64)

### `nodeSelector`

We can deploy Kata only to specific nodes using `nodeSelector`

```sh
# First, label the nodes where you want kata-containers to be installed
$ kubectl label nodes worker-node-1 kata-containers=enabled
$ kubectl label nodes worker-node-2 kata-containers=enabled

# Then install the chart with `nodeSelector`
$ helm install kata-deploy \
  --set nodeSelector.kata-containers="enabled" \
  "${CHART}" --version  "${VERSION}"
```

You can also use a values file:

```yaml title="values.yaml"
nodeSelector:
  kata-containers: "enabled"
  node-type: "worker"
```

```sh
$ helm install kata-deploy -f values.yaml "${CHART}" --version "${VERSION}"
```

### Multiple Kata installations on the Same Node

For debugging, testing and other use-case it is possible to deploy multiple
versions of Kata on the very same node. All the needed artifacts are getting the
`multiInstallSuffix` appended to distinguish each installation. **BEWARE** that one
needs at least **containerd-2.0** since this version has drop-in conf support
which is a prerequisite for the `multiInstallSuffix` to work properly.

```sh
$ helm install kata-deploy-cicd       \
  -n kata-deploy-cicd                 \
  --set env.multiInstallSuffix=cicd   \
  --set env.debug=true                \
  "${CHART}" --version  "${VERSION}"
```

Note: `runtimeClasses` are automatically created by Helm (via
      `runtimeClasses.enabled=true`, which is the default).

Now verify the installation by examining the `runtimeClasses`:

```sh
$ kubectl get runtimeClasses
NAME                            HANDLER                         AGE
kata-clh-cicd                   kata-clh-cicd                   77s
kata-clh-runtime-rs-cicd        kata-clh-runtime-rs-cicd        77s
kata-dragonball-cicd            kata-dragonball-cicd            77s
kata-fc-cicd                    kata-fc-cicd                    77s
kata-qemu-cicd                  kata-qemu-cicd                  77s
kata-qemu-coco-dev-cicd         kata-qemu-coco-dev-cicd         77s
kata-qemu-nvidia-gpu-cicd       kata-qemu-nvidia-gpu-cicd       77s
kata-qemu-nvidia-gpu-snp-cicd   kata-qemu-nvidia-gpu-snp-cicd   77s
kata-qemu-nvidia-gpu-tdx-cicd   kata-qemu-nvidia-gpu-tdx-cicd   76s
kata-qemu-runtime-rs-cicd       kata-qemu-runtime-rs-cicd       77s
kata-qemu-se-runtime-rs-cicd    kata-qemu-se-runtime-rs-cicd    77s
kata-qemu-snp-cicd              kata-qemu-snp-cicd              77s
kata-qemu-tdx-cicd              kata-qemu-tdx-cicd              77s
kata-stratovirt-cicd            kata-stratovirt-cicd            77s
```

## RuntimeClass Node Selectors for TEE Shims

**Manual configuration:** Any `nodeSelector` you set under `shims.<shim>.runtimeClass.nodeSelector`
is **always applied** to that shim's RuntimeClass, whether or not NFD is present. Use this when
you want to pin TEE workloads to specific nodes (e.g. without NFD, or with custom labels).

**Auto-inject when NFD is present:** If you do *not* set a `runtimeClass.nodeSelector` for a
TEE shim, the chart can **automatically inject** NFD-based labels when NFD is detected in the
cluster (deployed by this chart with `node-feature-discovery.enabled=true` or found externally):

- AMD SEV-SNP shims: `amd.feature.node.kubernetes.io/snp: "true"`
- Intel TDX shims: `intel.feature.node.kubernetes.io/tdx: "true"`
- IBM Secure Execution for Linux (SEL) shims (s390x): `feature.node.kubernetes.io/cpu-security.se.enabled: "true"`

The chart uses Helm's `lookup` function to detect NFD (by looking for the
`node-feature-discovery-worker` DaemonSet). Auto-inject only runs when NFD is detected and
no manual `runtimeClass.nodeSelector` is set for that shim.

**Note**: NFD detection requires cluster access. During `helm template` (dry-run without a
cluster), external NFD is not seen, so auto-injected labels are not added. Manual
`runtimeClass.nodeSelector` values are still applied in all cases.


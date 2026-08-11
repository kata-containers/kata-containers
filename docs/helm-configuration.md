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

```yaml
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

### defaultShim

`defaultShim` selects, per architecture, which shim the auto-created default
RuntimeClass (`runtimeClasses.createDefault`) resolves to:

```yaml title="values.yaml"
defaultShim:
  amd64: qemu-runtime-rs
  arm64: qemu-runtime-rs
  s390x: qemu-runtime-rs
  ppc64le: qemu
```

Since the Kata Containers **4.0 release**, the default is the **Rust runtime**
(`runtime-rs`, `qemu-runtime-rs`) on every architecture that ships a
`runtime-rs` build (x86_64, aarch64, s390x). ppc64le has no `runtime-rs` build
yet and stays on the Go runtime (`qemu`). The Go runtime remains selectable
(e.g. via the `kata-qemu` RuntimeClass) but is
[deprecated](migrating-config-go-runtime-to-runtime-rs.md#go-runtime-deprecation).
See the [config migration guide](migrating-config-go-runtime-to-runtime-rs.md)
for per-option differences when migrating.

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

### k8sDistribution

Different Kubernetes distributions keep containerd's configuration in different
places, so the chart needs to be told which one this cluster is. The value picks the
host directory that gets mounted into the install:

```yaml title="values.yaml"
k8sDistribution: k8s   # k8s | k3s | rke2 | k0s | microk8s
```

Any other value means the default location, `/etc/containerd/` — so `kubeadm` and
`vanilla` are as good as `k8s`. Set `containerd.configDir` instead if your
containerd matches none of the presets.

!!! warning "A wrong value fails the install rather than misconfiguring the node"

    Which *file* to write is worked out on the node itself, from the CRI runtime
    actually running there — so declaring `k8s` on a `k3s` cluster used to produce a
    perfectly good `k3s` configuration inside a directory `k3s` does not read, with
    nothing to show for it afterwards but a node that never runs a Kata workload.
    The install now compares the two and stops before writing anything, naming the
    value to set. An explicit `containerd.configDir` overrides the derivation this
    check is about, so it does not apply in that case.

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
  host-check -> artifacts   (initContainers)  ->  cri (main)
  ```

  On `helm uninstall`, a `pre-delete` dispatcher fans out per-node Jobs that run
  the pipeline in reverse (`revert-cri -> remove-artifacts`). Unlike
  the DaemonSet, **nothing keeps running on the node after installation
  completes**, and the dispatcher itself only ever talks to the API server — it
  never touches the host (so it ships as a separate, minimal image,
  `job.dispatcherImage`).

  The privilege split is explicit, and it is what `job` mode buys you over the
  DaemonSet: **the privileged pods hold no API credentials at all**. Everything
  that needs the API server is done by the dispatcher, which runs **fully
  unprivileged** (`runAsNonRoot`, all capabilities dropped, no privilege
  escalation, read-only root filesystem, `RuntimeDefault` seccomp), never touches
  the host, and can be confined to nodes you trust.

```yaml title="values.yaml"
deploymentMode: job
```

#### Where the credentials live

Only **one** identity exists in `job` mode, and it is not the one that runs on
your nodes. This matters because root on a node can read the ServiceAccount token
of any pod running there — so the per-node Jobs are rendered with
`automountServiceAccountToken: false` and no ServiceAccount of their own.

| | runs where | privileged on the host | API rights |
|---|---|---|---|
| dispatcher (install, uninstall) | one pod, only where you let it schedule ([Where the dispatcher runs](#where-the-dispatcher-runs)) | no | `nodes: list, get, patch`; Jobs in the release namespace; `nodes/proxy: get` only when guest pull or image conversion is configured |
| per-node Jobs | every node Kata is installed on | yes | **none — no token is mounted** |

Getting there moved three things out of the per-node Jobs:

- **Labelling the node and lifting its taints.** The dispatcher applies
  `katacontainers.io/kata-runtime` once a node's Job has succeeded *and* the node
  reports `Ready` again (`job.waitNodeReadySeconds`), then lifts the configured
  `startupTaints`. This is also a stronger guarantee than before: the label can no
  longer appear on a node whose install pipeline failed halfway, and it is removed
  *before* an uninstall Job starts taking the node apart.
- **The one node fact the install cannot work out locally** — the node's container
  runtime version — is read by the dispatcher off the `Node` objects it already
  listed, and passed down as an environment variable. Everything else it needs
  about the node, the Kubernetes flavour included, it establishes by asking the
  host.
- **The cluster-scoped objects**: the `RuntimeClass`es and the `NodeFeatureRule`
  advertising TEE key counts are rendered by the chart (see
  [TEE key advertisement](#tee-key-advertisement)) rather than applied at run time
  from a root-on-the-host container. `helm uninstall` removes them for free.

!!! note "How this compares to `daemonset` mode"

    Installing Kata means writing to the host and restarting the CRI runtime, so
    something privileged has to run on each node in either mode. Two things differ.
    The DaemonSet pod carries the `kata-deploy` ServiceAccount, which can patch any
    node in the cluster and read any node's kubelet configuration, and it stays on
    the node — token and all — for the life of the release. A per-node Job carries
    no token and exits when it is done.

    So in `job` mode a compromised worker node yields no cluster credentials, and
    the one component that does hold them can be kept on the control plane. In
    `daemonset` mode that token is, by construction, on every node Kata runs on.

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

### Where the dispatcher runs

The dispatcher holds the one token in `job` mode that can enumerate every node
in the cluster and create the privileged per-node Jobs, and root on whatever
node it lands on can read it. Confine it to nodes you trust — typically the
control plane — with `job.dispatcherNodeSelector` and
`job.dispatcherTolerations`:

```yaml title="values.yaml"
job:
  dispatcherNodeSelector:
    node-role.kubernetes.io/control-plane: ""
  dispatcherTolerations:
    - key: node-role.kubernetes.io/control-plane
      operator: Exists
      effect: NoSchedule
```

The toleration is needed because control-plane nodes are normally tainted. These
settings say nothing about where Kata is installed — that remains the top-level
`nodeSelector` / `affinity` / `tolerations`, and the per-node Jobs do not inherit
the dispatcher's placement. When `job.dispatcherTolerations` is empty it falls
back to the top-level `tolerations`, so the dispatcher stays schedulable on a
cluster whose every node is tainted.

!!! tip "Pinning covers every credential in the release"

    The dispatcher holds the only token `job` mode has, so keeping it off your
    workers leaves no Kubernetes credential on them at all — the privileged
    per-node Jobs mount none (see
    [Where the credentials live](#where-the-credentials-live)).

    That is a boundary `daemonset` mode cannot draw, since the pod holding its
    token has to run on every node by definition.

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

Node selection is **not** configured per deployment mode. Both modes read the
same [`nodeSelector`](#nodeselector) / [`affinity`](#affinity) and
[`tolerations`](#tolerations), so one values file installs Kata on the same nodes
whichever mode you deploy with.

In `daemonset` mode the scheduler applies those rules when it places the
kata-deploy pods. In `job` mode there is no pod to place — the dispatcher pins
each per-node Job to a node by name — so the dispatcher applies the same rules
itself while enumerating nodes:

1. `nodeSelector` and `affinity.nodeAffinity` are compiled into node queries, which
   the dispatcher resolves live against the API server. Each `nodeSelectorTerm`
   becomes one query and their results are combined with OR, matching Kubernetes'
   own semantics; the `nodeSelector` labels are ANDed into every query, which is
   how Kubernetes combines the two.
2. Nodes carrying a taint your `tolerations` do not cover are then dropped,
   exactly as the scheduler would refuse to place a DaemonSet pod there.
3. `job.nodes` overrides all of the above with an explicit list of node names,
   used verbatim.

An empty `nodeSelector` therefore does **not** mean "every node": step 2 still
applies, and that is what keeps Kata off control-plane nodes by default without
any label filtering.

```yaml title="values.yaml"
# Install on nodes carrying a specific label:
nodeSelector:
  kata-containers: "enabled"

# Richer selection - Linux workers that are either GPU- or SNP-capable
# (a node matching either term is selected):
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - { key: kubernetes.io/os, operator: In, values: ["linux"] }
            - { key: nvidia.com/gpu.present, operator: In, values: ["true"] }
        - matchExpressions:
            - { key: kubernetes.io/os, operator: In, values: ["linux"] }
            - { key: feature.node.kubernetes.io/cpu-cpuid.SEV_SNP, operator: In, values: ["true"] }

# Pin to explicit nodes (job mode only; tainted nodes included, since naming a
# node is unambiguous):
job:
  nodes: ["worker-1", "worker-2"]
```

!!! warning "The `job.*` node-selection keys have been removed"
    Earlier releases selected nodes with `job.nodeSelector`,
    `job.nodeSelectorExpressions` and `job.nodeAffinity`, separately from the
    DaemonSet's own knobs. Having two sources of truth meant a values file that
    pinned Kata to a few nodes in `daemonset` mode could silently install it
    everywhere in `job` mode. Rendering now **fails with a migration hint** if any
    of those keys is still set: move the rule to the top-level `nodeSelector` or
    `affinity.nodeAffinity`, and keep `job.nodes` for explicit node names.

!!! note "Not every affinity rule can be expressed as a node query"
    Job mode resolves nodes with a label-selector `LIST` against the API server,
    so `matchFields` and the `Gt`/`Lt` operators cannot be represented and are
    rejected at render time — use `job.nodes` to target nodes by name.
    `preferredDuringSchedulingIgnoredDuringExecution` is accepted but ignored: a
    preference never restricts where a DaemonSet may land either, so honouring
    only the required terms keeps both modes in agreement.

Use `job.parallelism` to pace the rollout — it caps how many per-node Jobs run
concurrently (e.g. to limit how many CRI runtimes restart at once on a big
fleet). It is effectively capped at the number of targeted nodes.

#### Nodes that become eligible late

A DaemonSet is level-triggered: it installs a node whenever that node becomes
eligible, however long that takes. The dispatcher instead resolves nodes once,
at install time, and eligibility frequently arrives seconds later — the
`feature.node.kubernetes.io/*` labels that `affinity.nodeAffinity` matches are
written by node-feature-discovery, which the chart can install in the very same
release, and a freshly joined node clears its start-up taints only once it
settles.

So the install dispatcher keeps re-resolving for `job.waitForNodesSeconds`
(default `120`) while nothing is eligible yet, and fails if the wait expires
with no eligible node. Failing is deliberate: in `job` mode an empty selection
is permanent — nothing installs the node later — so a silent success would leave
the fleet without Kata and nothing to point at.

```yaml title="values.yaml"
job:
  # Give a slow-labelling cluster longer (see the timeout warning below)...
  waitForNodesSeconds: 300
  # ...or resolve once and treat "no eligible node" as a no-op, the way a
  # DaemonSet with no matching node quietly installs nothing.
  # waitForNodesSeconds: 0
```

!!! warning "Raise `helm --timeout` alongside it"
    The wait is spent inside the dispatcher hook, and Helm bounds that hook by
    the release timeout (`--timeout`, 5 minutes by default) — the same budget
    that has to cover extracting the artifacts and restarting the CRI runtime on
    every node. Raising `waitForNodesSeconds` on its own only trades a
    dispatcher error naming the selectors it tried for Helm's opaque
    `timed out waiting for the condition`, which leaves the release failed and
    the dispatcher Job orphaned.

The uninstall dispatcher never waits: it has to run to completion on a release
that may never have labeled a node.

#### Installing on control-plane nodes

Control-plane nodes are normally tainted, which is what keeps Kata off them in
both modes. Reaching them takes a matching toleration — and, if you want *only*
those nodes, a selector as well:

=== "Control-plane nodes only"
    ```yaml title="values.yaml"
    nodeSelector:
      node-role.kubernetes.io/control-plane: ""
    tolerations:
      - key: node-role.kubernetes.io/control-plane
        operator: Exists
        effect: NoSchedule
    ```

=== "Control-plane nodes and workers"
    ```yaml title="values.yaml"
    # No selector: every node is eligible, and the toleration lets the tainted
    # control-plane nodes through as well.
    tolerations:
      - key: node-role.kubernetes.io/control-plane
        operator: Exists
        effect: NoSchedule
    ```

!!! tip "Single-node clusters usually need nothing"
    Single-node distributions (k3s, k0s `--single`) do not taint their node, and
    `kubeadm` clusters used for local testing are typically untainted with
    `kubectl taint nodes --all node-role.kubernetes.io/control-plane-`. In both
    cases the node is selected by default. On older clusters the label and taint
    key may be `node-role.kubernetes.io/master` instead.

In `job` mode, a node skipped for a taint it does not tolerate is named in the
dispatcher log along with the taint responsible, so an unexpectedly small rollout
is easy to explain:

```sh title="$ kubectl logs job/kata-deploy-install-dispatcher"
skipping node cp-1: it carries the taint node-role.kubernetes.io/control-plane:NoSchedule,
which this install does not tolerate (a DaemonSet would not have been scheduled there either)
```

If *every* selected node is skipped this way the dispatcher fails rather than
reporting success with nothing done, and its error quotes the toleration to add.

### Choosing which nodes are cleaned up on uninstall

Because the cleanup dispatcher resolves nodes **live when it runs** at
`helm uninstall` (the dispatcher does the lookup, not Helm at render time), the
node set is *not* frozen into the stored release. This means the **default
cleanup selector can simply be "nodes carrying the
`katacontainers.io/kata-runtime` label"** — i.e. exactly the nodes the install
touched, regardless of how the install selector has drifted since.

The selector matches the label's *key*, not the value `"true"`, and that is
deliberate. An install claims its node with `katacontainers.io/kata-runtime=false`
before it writes anything to it, and only promotes it to `"true"` once the node
is kata-capable. Uninstall therefore also reaches nodes where the install failed
part way through — the ones most likely to have artifacts or CRI configuration
left on them. Nothing schedules onto a claimed node in the meantime, since the
RuntimeClasses select the exact value `"true"`.

Override it under `job.cleanup` (`cleanup.nodes`, then `cleanup.nodeSelector`
ANDed with `cleanup.nodeAffinity`, else all nodes):

```yaml title="values.yaml"
# Only uninstall from specific nodes:
job:
  cleanup:
    nodes: ["worker-1"]

# Use an explicit selector instead of the kata-runtime label default:
job:
  cleanup:
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
              - { key: node-role.kubernetes.io/control-plane, operator: DoesNotExist }
```

!!! note "Uninstall is deliberately independent of install"
    Cleanup is **not** narrowed by the top-level `nodeSelector`/`affinity`, and
    tainted nodes are cleaned rather than skipped. It has to be able to revert
    every node the install ever touched, even one whose labels or taints have
    changed since — so it targets what was installed, not what is selected now.

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

### [`try-kata-nvidia-cpu.values.yaml`](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-nvidia-cpu.values.yaml)

This file enables only the NVIDIA CPU-only shims and installs them using the
[`job` deployment mode](#deployment-modes-daemonset-vs-job) (no always-on
DaemonSet on the node):

```sh
helm install kata-deploy oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy \
  --version VERSION \
  -f try-kata-nvidia-cpu.values.yaml
```

Includes:

- `qemu-nvidia-cpu` - NVIDIA base image without GPU passthrough (amd64, arm64)
- `qemu-nvidia-cpu-runtime-rs` - NVIDIA base image without GPU passthrough,
  using the Rust runtime (amd64, arm64)

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

```yaml
nodeSelector:
  kata-containers: "enabled"
  node-type: "worker"
```

```sh
$ helm install kata-deploy -f values.yaml "${CHART}" --version "${VERSION}"
```

!!! info "Applies to both deployment modes"
    `nodeSelector` selects nodes in `daemonset` and `job` mode alike — see
    [Choosing which nodes get a Job](#choosing-which-nodes-get-a-job) for how job
    mode applies it. Note that leaving it empty does not mean "every node": nodes
    carrying a taint you do not tolerate are still excluded, which is what keeps
    Kata off control-plane nodes.

!!! info "Combining it with `affinity.nodeAffinity`"
    Both may be set at once and are **ANDed**, exactly as they would be on any pod
    spec: a node has to satisfy the `nodeSelector` labels *and* match one of the
    `nodeSelectorTerms`. Job mode reproduces this by folding the `nodeSelector`
    equalities into every term, so the two modes select the same nodes.

### `tolerations`

Tolerations decide which **tainted** nodes Kata may be installed on. Selecting a
node is not enough on its own: if the node carries a `NoSchedule` or `NoExecute`
taint that your tolerations do not cover, it is excluded — in `daemonset` mode by
the scheduler, and in `job` mode by the dispatcher applying the same rule.

That default is what keeps Kata off control-plane nodes, and off nodes your
platform has reserved for something else, without you having to exclude them by
label.

```yaml title="values.yaml"
tolerations:
  - key: "reservedFor"
    operator: "Exists"
    effect: "NoSchedule"
```

To install on control-plane nodes, see
[Installing on control-plane nodes](#installing-on-control-plane-nodes).

!!! note "Cordoned and pressured nodes are still installed on"
    Kubernetes gives every DaemonSet pod an implicit set of tolerations so that
    node conditions — `unschedulable` (a cordoned node), plus disk, memory and PID
    pressure — never stop a node agent from running. Job mode applies the same
    implicit set, so cordoning a node does not make it silently miss the install
    in either mode.

### `podLabels`

You can add extra labels to the kata-deploy DaemonSet pods. These are applied
in addition to the `name: kata-deploy` label that the chart uses internally.
The chart ignores a `name` key in `podLabels` and always sets the required
selector label itself.

```sh
$ helm install kata-deploy \
  --set podLabels.team=platform \
  "${CHART}" --version "${VERSION}"
```

Or via a values file:

```yaml
podLabels:
  team: platform
```

### `podAnnotations`

You can add annotations to the kata-deploy DaemonSet pods for whatever your
environment needs: Prometheus scrape hints, policy markers, revision metadata,
and so on. The chart does not set any annotations on the kata-deploy DaemonSet
pod template by default, so nothing is reserved or overwritten unless you set
`podAnnotations` yourself.

```sh
$ helm install kata-deploy \
  --set-string podAnnotations.prometheus\.io/scrape=false \
  "${CHART}" --version "${VERSION}"
```

Or via a values file:

```yaml
podAnnotations:
  prometheus.io/scrape: "false"
  example.com/owner: platform-team
```

### `affinity`

Use `affinity` when you need more granular scheduling controls than
`nodeSelector` alone. `nodeSelector` only matches exact key/value pairs on a
node; affinity gives you `matchExpressions` (e.g. `In`, `NotIn`) and rules
about other pods on the same node. For example, you might want kata-deploy on
nodes reserved for your platform team but *not* on nodes that run the GPU
operator.

!!! info "`nodeAffinity` selects nodes in both modes; pod affinity only in `daemonset` mode"
    `affinity.nodeAffinity` is the match-expression form of `nodeSelector` and, like
    it, decides which nodes get Kata in **both** deployment modes. The two can be
    combined, and are ANDed. `podAffinity`/`podAntiAffinity` describe how to
    *schedule a pod*, so they only apply in `daemonset` mode; job mode pins each
    per-node Job to a node by name and has no scheduling decision to influence. See
    [Choosing which nodes get a Job](#choosing-which-nodes-get-a-job) for which
    `nodeAffinity` constructs job mode can and cannot express.

```sh
# First, label the nodes where kata-deploy should run
$ kubectl label nodes worker-node-1 node.cloud/reserved=platform-team
$ kubectl label nodes worker-node-2 node.cloud/reserved=platform-team

# Then install the chart with affinity
$ helm install kata-deploy -f values.yaml "${CHART}" --version "${VERSION}"
```

```yaml
affinity:
  nodeAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      nodeSelectorTerms:
        - matchExpressions:
            - key: node.cloud/reserved
              operator: In
              values:
                - platform-team
  podAntiAffinity:
    requiredDuringSchedulingIgnoredDuringExecution:
      - labelSelector:
          matchExpressions:
            - key: app
              operator: In
              values:
                - gpu-operator
        topologyKey: kubernetes.io/hostname
```

When `node-feature-discovery.enabled=true`, the chart also merges in a
`nodeAffinity` rule that requires hardware virtualization support.

!!! note "How NFD and user nodeAffinity are combined"
    Within a single `nodeSelectorTerm`, `matchExpressions` and `matchFields` are
    **AND**-ed (all must match). Across `nodeSelectorTerms`, terms are **OR**-ed
    (any one term may match).

    If you set `affinity.nodeAffinity` yourself, only your **required**
    `nodeSelectorTerms` participate in the merge. They are combined with the
    built-in virtualization terms as **(NFD OR-group) AND (user OR-group)**:
    each built-in term is AND-ed with each of your required terms. Multiple
    user required terms remain OR among themselves. NFD virtualization
    requirements cannot be bypassed by user affinity.

    If you set `nodeAffinity` without `requiredDuringSchedulingIgnoredDuringExecution`,
    the built-in NFD required terms are still applied. Other affinity fields
    (`podAffinity`, `podAntiAffinity`, and `preferredDuringSchedulingIgnoredDuringExecution`)
    are passed through unchanged.

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

## TEE key advertisement

A confidential VM consumes a *hardware key slot* — an encrypted-state ID on AMD
SEV-SNP, a key ID on Intel TDX — and a node has a small, fixed number of them. The chart models that
as an extended resource so the scheduler stops placing confidential pods on a node
whose slots are all taken, instead of letting the VM fail to start:

- a `NodeFeatureRule` tells node-feature-discovery to advertise the per-node counts
  as `sev-snp.amd.com/esids` and `tdx.intel.com/keys`
- the matching `RuntimeClass`es request one of them in `overhead.podFixed`, so every
  pod that uses the class consumes a slot

Both halves are one switch, because requesting a resource nothing advertises would
leave confidential pods `Pending` forever:

```yaml title="values.yaml"
nodeFeatureRules:
  create: auto   # auto | true | false
```

`auto` renders them when NFD is in the picture: installed by this chart
(`node-feature-discovery.enabled=true`), already present in the cluster, or its CRD
is registered. `true` and `false` decide outright — `false` is the escape hatch for
a cluster that manages the rule itself.

!!! note "This used to be the install binary's job"
    The rule and the `RuntimeClass` patching were applied at run time by the
    privileged pod on each node, which meant granting cluster-wide write access to
    `nodefeaturerules` and `runtimeclasses` to a container running as root on the
    host — once per node, for a pair of cluster-scoped objects. Rendering them in
    the chart drops that grant, does it once per release, and removes them on
    `helm uninstall`. The rule is named `kata-tee-keys` (suffixed with
    `env.multiInstallSuffix` when set); a rule left behind by an older release is
    called `amd64-tee-keys` and can be deleted once every release has been upgraded.

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

### NVIDIA guest settings

The NVIDIA GPU images boot [NVRC](https://github.com/NVIDIA/nvrc) as their init
process, which brings the NVIDIA stack up before the Kata agent starts. The
`shims.<shim>.nvrc` block configures it. Every key becomes an `nvrc.*` guest
kernel parameter, so it applies to all sandboxes on that shim and takes effect
when a sandbox boots:

```yaml title="values.yaml"
shims:
  qemu-nvidia-gpu:
    nvrc:
      enableDCGM: true
```

`enableDCGM` runs `nv-hostengine` and `dcgm-exporter` inside the guest. The
exporter serves GPU metrics on port 9400 of the sandbox, which is the pod IP, so
anything that can reach the pod can scrape `/metrics` without a sidecar:

```sh
curl "http://$(kubectl get pod my-gpu-pod -o jsonpath='{.status.podIP}'):9400/metrics"
```

It is off by default because it costs guest memory and an extra process in every
sandbox on the shim. Enable it per shim, on the shims whose workloads you want to
observe.

!!! note
    The block is only meaningful on the `qemu-nvidia-gpu*` shims, and kata-deploy
    refuses to install if it finds it on any other shim. DCGM itself ships in the
    NVIDIA GPU guest images — with composable images it arrives in the GPU
    extension — so a shim using a guest image without it will not serve metrics.

Under the hood kata-deploy appends `nvrc.dcgm=on` to the shim's kernel command
line, in the same `config.d/30-kernel-params.toml` drop-in that carries the proxy
and debug settings.

### defaultShim

`defaultShim` selects, per architecture, which shim the auto-created default
RuntimeClass (`runtimeClasses.createDefault`) resolves to:

```yaml title="values.yaml"
defaultShim:
  amd64: qemu-runtime-rs
  arm64: qemu-runtime-rs
  s390x: qemu-runtime-rs
  ppc64le: qemu-runtime-rs
```

Since the Kata Containers **4.0 release**, the default is the **Rust runtime**
(`runtime-rs`, `qemu-runtime-rs`) on every architecture that ships a
`runtime-rs` build (x86_64, aarch64, s390x and ppc64le). The Go runtime remains selectable
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

!!! warning "A wrong value stops the install before changing the node"

    This value picks the *directory*, while which *file* to write is worked out on
    the node itself, from the CRI runtime actually running there. Declaring `k8s` on
    a `k3s` cluster therefore describes a perfectly good `k3s` configuration inside a
    directory `k3s` does not read — a node that never runs a Kata workload, with
    nothing to show for it. The install compares the two and stops before writing
    anything, naming the value to set. An explicit `containerd.configDir` overrides
    the derivation this check is about, so it does not apply in that case.

### nodeBinaries

Some of what Kata needs on a node is not part of Kata: containerd's EROFS
snapshotter, for instance, needs a `mkfs.erofs` from `erofs-utils` 1.8.2 or newer,
which most distributions still do not package. When updating the node's packages is
not on offer, `nodeBinaries` takes the binaries out of container images instead:

```yaml title="values.yaml"
nodeBinaries:
  erofs-utils:                                      # (1)!
    image: quay.io/kata-containers/erofs-utils:1.9.3
    binaries: [mkfs.erofs, dump.erofs, fsck.erofs]  # (2)!
    pullPolicy: IfNotPresent                        # (3)!
```

1. Each key names an entry and becomes the name of the container staging it, so it
   has to be a valid container name — lowercase letters, digits and dashes,
   beginning and ending with a letter or a digit — and cannot be one of the names
   kata-deploy's own containers use. The render fails, naming the key, when it is
   neither.
2. Only what is listed here is taken, so an image built on a distribution does not
   put the rest of its userland on the node. Each one names a single binary, not a
   path or a pattern, and the render fails on anything else.
3. Optional; defaults to the chart's `imagePullPolicy`.

Every entry gets a container of its own, which copies the listed binaries into a
pod-local volume and reaches nothing of the node's. One further container then
installs whatever was staged into `/usr/local/bin`, ahead of `/usr/bin` in
containerd's `PATH`, and records the names it installed so that a later run can take
exactly those out again. That container runs the `kubectl` image, kata-deploy's own
being distroless and having no shell to do this with.

Each image needs a POSIX shell, `cp`, and every binary the entry lists, statically
built, in one of `/usr/local/bin`, `/usr/local/sbin`, `/usr/bin`, `/usr/sbin`,
`/bin`, `/sbin` or its root. Private images use the chart's `imagePullSecrets`.

Adding another binary is a values change and nothing else, so this is the way to
cover anything else a node turns out to lack.

!!! warning "It will not replace a binary it did not install"

    A file already in `/usr/local/bin` under a name an entry claims fails the
    install, rather than being replaced, and the same goes for two entries claiming
    the same name. Nothing on the node records where a binary in `/usr/local/bin`
    came from, so kata-deploy only ever removes what its own marker file names.
    Remove the file, or drop it from `nodeBinaries` to keep using it.

    An install it refuses this way changes nothing: every name is checked before any
    of them is written or removed, so the node keeps the set it already had.

An uninstall takes the binaries out again, and changing an entry replaces what it
installed. Dropping every entry leaves them in place until the release is
uninstalled.

!!! note "Side-by-side installs own separate sets"

    Each release's marker file is named after its `env.multiInstallSuffix`, so
    installing or uninstalling one leaves the binaries another one installed alone.
    Two releases claiming the same name is still a conflict: whichever installs
    second finds a file it did not install and fails.

!!! note "One image per architecture being deployed to"

    An image with no manifest for a node's architecture stalls that node's install
    on the pull, so cover every architecture in the cluster. The `erofs-utils` image
    above is published for `amd64` and `arm64` only.

This requires `deploymentMode: job`. The staged pipeline is what puts the binaries
in place before the host check looks for them; the DaemonSet runs the whole install
in one container and has no such ordering. Setting `nodeBinaries` in `daemonset` mode
fails the render rather than deploying something that cannot work.

## Deployment Modes (DaemonSet vs Job)

The chart can install Kata on nodes in one of two ways, selected with the
top-level `deploymentMode` value:

- **`daemonset`**: the long-running `kata-deploy` DaemonSet installs
  Kata on every matching node and reverts it when the pod is terminated (i.e. on
  uninstall). This is the historical behavior and is unchanged.
- **`job`** (default): there is **no always-on component**. A tiny
  [`k8s-job-dispatcher`](https://github.com/kata-containers/k8s-job-dispatcher)
  Job runs as a `post-install`/`post-upgrade` hook, enumerates the selected nodes
  **live** via the Kubernetes API, and creates one
  node-pinned install `Job` per node. Each per-node Job runs the staged install
  pipeline as ordered `initContainers` and then exits:

  ```text
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

`job` is the default; ask for `daemonset` explicitly to keep the historical
behavior:

=== "Job (default)"
    ```yaml title="values.yaml"
    deploymentMode: job
    ```

=== "DaemonSet"
    ```yaml title="values.yaml"
    deploymentMode: daemonset
    ```

!!! warning "Upgrading a release installed before `job` became the default"
    `deploymentMode` is [immutable for the life of a release](#how-installations-keep-out-of-each-others-way-on-a-node),
    so an existing `daemonset` release does not silently move to per-node Jobs:
    the upgrade is refused instead. Keep `deploymentMode: daemonset` in your
    values to stay where you are, or `helm uninstall` first to adopt `job`.

#### Where the credentials live

The pods that run on your nodes hold no API credentials at all: the per-node Jobs
are rendered with `automountServiceAccountToken: false` and no ServiceAccount of
their own. This matters because root on a node can read the ServiceAccount token
of any pod running there.

| | runs where | privileged on the host | API rights |
|---|---|---|---|
| dispatcher (install, uninstall, and each scheduled reconcile) | one pod, only where you let it schedule ([Where the dispatcher runs](#where-the-dispatcher-runs)) | no | `nodes: list, get, patch`; `jobs: create, get, list, delete` and `pods: list` in the release namespace; `events: create` in `default`, which is the only namespace an Event about a Node can live in; `nodes/proxy: get` only when guest pull or image conversion is configured; `pods: get` and `cronjobs: get, delete` only with [`job.reconcile`](#doing-it-on-a-schedule-instead) enabled |
| per-node Jobs | every node Kata is installed on | yes | **none — no token is mounted** |
| `post-delete` hook (uninstall only) | one pod, wherever it schedules | no | `delete` on ClusterRoles, ClusterRoleBindings, Roles, RoleBindings and ServiceAccounts |
| verification Job (only if `verification.pod` is set) | one pod, wherever it schedules | no | pods and pod logs (`create`, `delete`, `get`, `list`, `watch`), `nodes`/`events`/`daemonsets`/`jobs`: `get`, `list` |

The last two rows are the exceptions worth knowing about, and both exist in either
mode. The `post-delete` hook deletes the RBAC the release keeps, so its own rights
have to outlive everything else; the verification Job runs the pod you gave it and
reports what happened. Both are short-lived — one runs during `helm uninstall`, the
other after `helm install`/`helm upgrade` — but neither is pinned, so for those few
seconds their tokens can be on any schedulable node. The verification Job is opt-in:
leave `verification.pod` unset and it does not exist.

Three things need the Kubernetes API, so none of them happen inside a per-node Job:

- **Labelling the node, and everything that gates it.** The dispatcher claims a node
  with `katacontainers.io/kata-runtime=false` before its Job starts. It promotes the
  label to `true` only once three things hold: that Job succeeded, the node reports
  `Ready` again (`job.waitNodeReadySeconds`), and the node's own
  `.status.runtimeHandlers` confirms its CRI runtime is serving the handlers this
  release installed. Only then does it lift the configured `startupTaints`. A pod on
  the node could not offer any of that. The label cannot appear on a node whose
  pipeline failed half way, nor on one whose runtime quietly ignored the
  configuration that was written. It survives a kubelet that re-registers after the
  runtime restart and drops it, because the dispatcher re-applies it until it holds.
  On uninstall the value drops to `false`, which stops workloads being scheduled,
  *before* that node's Job starts taking the node apart — and the label itself goes
  only once that Job has succeeded.

    In `job` mode the claim is mandatory: the dispatcher refuses to start a
    host-mutating Job unless both labels were written atomically. Otherwise a
    half-failed install could modify a host that the default uninstall selector
    cannot discover. In `daemonset` mode the claim stays best-effort.

    The handler check has one limit worth knowing: `.status.runtimeHandlers` is
    populated by kubelet from Kubernetes 1.30 on. A node that does not report the
    field at all cannot answer, and is labelled on the strength of its Job
    succeeding. A node that reports the field without any of this release's handlers
    in it fails, and an empty list counts the same way: that node has answered, it
    just has nothing this release installed. One handler is enough to pass, because
    the release names the handlers for every architecture it can install and a node
    only serves its own; the ones it does not serve are logged. A release that
    registers no handler names — every shim disabled and no custom runtime — has
    nothing to ask about, and skips the check.
- **The node's container runtime version.** It is the one fact the install cannot
  work out on the host itself, so the dispatcher reads it from the `Node` objects it
  has already listed and passes it down as an environment variable. Everything else
  the install needs to know about its node, the Kubernetes flavour included, it gets
  by asking the host.
- **The cluster-scoped objects**: the `RuntimeClass`es and the `NodeFeatureRule`
  advertising TEE key counts are rendered by the chart (see
  [TEE key advertisement](#tee-key-advertisement)), so nothing on a node needs
  write access to them. `helm uninstall` removes them for free.

!!! note "How this compares to `daemonset` mode"

    Installing Kata means writing to the host and restarting the CRI runtime, so
    something privileged has to run on each node in either mode. Two things differ.
    The DaemonSet pod carries the `kata-deploy` ServiceAccount, which can patch any
    node in the cluster and read any node's kubelet configuration, and it stays on
    the node — token and all — for the life of the release. A per-node Job carries
    no token and exits when it is done.

    So in `job` mode a compromised worker node yields no cluster credentials, unless
    the `post-delete` hook or the verification Job happens to be running there at that
    moment. The component that does hold the node-patching token can be kept on the
    control plane. In `daemonset` mode that token is, by construction, on every node
    Kata runs on, for the life of the release.

!!! warning "Switching an existing `daemonset` release to `job` mode"

    The privileged `kata-deploy` ServiceAccount, ClusterRole and ClusterRoleBinding
    are annotated `helm.sh/resource-policy: keep`, so that a DaemonSet pod being
    terminated still has the API access its own cleanup needs. `keep` also means
    Helm leaves them behind on an upgrade that no longer renders them — which is
    exactly what switching to `job` mode does. The release stops using them, but the
    credential that can patch any node in the cluster is still there.

    They are cluster-scoped and un-suffixed, so a release in another namespace may
    share them; the chart cannot safely delete them for you. Once no `daemonset`
    release is left, remove them by hand:

    ```sh
    kubectl delete clusterrolebinding kata-deploy-rb
    kubectl delete clusterrole kata-deploy-role
    kubectl delete serviceaccount kata-deploy-sa -n <release namespace>
    ```

    With `env.multiInstallSuffix` set, the names carry that suffix.

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

!!! tip "What pinning does and does not cover"

    Pinning the dispatcher keeps the release's one cluster-wide, long-lived
    identity — the one that can enumerate and patch every node — off your workers.
    The privileged pods that do run on every node mount no token at all.

    It does not pin the two short-lived hooks: the `post-delete` RBAC cleanup and,
    if you set `verification.pod`, the verification Job (see
    [Where the credentials live](#where-the-credentials-live)). Both can land on
    any schedulable node for as long as they run.

    That is still a boundary `daemonset` mode cannot draw, since the pod holding
    its node-patching token has to run on every node by definition.

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

#### Doing it on a schedule instead

On a fleet that grows on its own — an autoscaler, or nodes joining faster than
anybody notices — the upgrade above needs somebody to run it, and until they do the
new node adds no Kata capacity. Nothing lands on it wrongly (every RuntimeClass
selects `katacontainers.io/kata-runtime`, which is exactly what the install has not
written there yet), so a pod asking for a Kata runtime class stays `Pending` while a
node that could have run it takes non-Kata work instead — and if it was the pending
pod that grew the fleet in the first place, the next node changes nothing either.
`job.reconcile` turns that upgrade into a `CronJob` running the same dispatcher,
against the same selectors and the same per-node templates:

```yaml title="values.yaml"
job:
  reconcile:
    enabled: true
    schedule: "*/15 * * * *"
```

A tick installs the nodes that have nothing to show yet and leaves the rest of the
fleet untouched, so a tick over a settled fleet lists nodes, finds nothing to do,
and exits without creating a single pod. It also stands aside when a release
rollout is in flight, rather than deleting the per-node Jobs that rollout is
waiting on. Failures are visible where you would look for them: the CronJob keeps
its last three failed Jobs, and a tick that failed on a node fails as a Job rather
than being retried immediately — the next tick is the retry.

!!! note "It only ever adds nodes"

    A node that has dropped out of the selection is left alone, where `helm
    upgrade` would run the cleanup pipeline on it. That asymmetry is deliberate: a
    node falling out is usually a label gone wrong somewhere, and taking a host
    apart on a timer with nobody watching is the worse of the two outcomes. Removals
    stay with the upgrade you run yourself.

!!! warning "Off by default"

    Enabling this stands up a recurring, privileged rollout, so it is opt-in. It
    also needs a `job.dispatcherImage` of `0.2.0` or newer; older dispatchers do
    not understand the flags a scheduled run needs and every tick fails at startup.

`helm uninstall` takes the schedule away before it starts reverting nodes (a
`pre-delete` hook that runs ahead of the uninstall dispatcher and removes the
CronJob together with anything it still has in flight), so a tick cannot reinstall
a node the uninstall has just cleaned.

### Recovering from a failed or deleted dispatcher

The dispatcher runs as a **blocking** `post-install`/`post-upgrade` hook Job with
`restartPolicy: Never` and `backoffLimit: 0`, so if its pod is evicted, drained,
or deleted mid-rollout the Job is marked *failed* and is **not** restarted
automatically — `helm install`/`helm upgrade` surfaces the failure rather than
leaving you silently half-installed.

What survives the dispatcher dying:

- **Per-node Jobs already created keep running.** They are independent,
  `nodeName`-pinned Jobs, not children of the dispatcher pod, so installs that
  were already dispatched run to completion. Only nodes still queued (never
  dispatched) are skipped, so at worst you get *partial coverage* — never a
  half-mutated host, because each stage is idempotent.
- **Those nodes are not labelled**, though. Labelling is the dispatcher's job (it
  is what lets the per-node Jobs run without a token), so a node whose install
  finished after the dispatcher died is left installed but not advertised, and no
  Kata workload is scheduled there. It still carries
  `katacontainers.io/kata-runtime=false` from being claimed, so `helm uninstall`
  reaches it, and the re-run below labels it.
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

### Finding out why a node failed

A rollout that fails says which nodes failed and what stopped each of them in
three places, as long as the dispatcher lived long enough to say so. Helm prints
its summary directly, and the node Event and annotation remain available after
the command exits.

=== "From `helm`"
    The chart asks Helm to print the dispatcher's log when the hook fails, and
    the dispatcher ends that log with one line per failed node, so the command
    that failed is also the one that explains it:

    ```text title="$ helm install kata-deploy ..."
    level=INFO msg="[2026-09-03T18:22:11Z INFO ] fanning out 2 per-node Job(s) with parallelism 1\n...
    Error: 2 node(s) failed:\n  worker-2: its job kata-deploy-install-worker-2 failed:
    load-kernel-modules exited 1: this node has no usable virtualization backend: neither
    /dev/kvm nor /dev/mshv is present [BackoffLimitExceeded: Job has reached the specified
    backoff limit]\n  worker-5: its job kata-deploy-install-worker-5 failed: cri never
    started: ImagePullBackOff: Back-off pulling image \"kata-deploy:bad-tag\"\n"
    Error: INSTALLATION FAILED: failed post-install: resource Job/kube-system/kata-deploy-install-dispatcher not ready. status: Failed, message: Job Failed. failed: 1/1
    ```

    !!! warning "Helm prints hook logs as a single escaped line"
        Helm writes hook output through its structured logger, so the whole log
        arrives as one `level=INFO msg="..."` record with newlines escaped to `\n`.
        Helm's own closing `Error:` line says only that the hook Job failed — the
        reason is inside that record. The dispatcher keeps its output short so the
        record stays readable, and `sed` gives it back its newlines:

        ```sh
        helm install kata-deploy "${CHART}" 2>&1 | sed -e 's/\\n/\n/g' -e 's/\\"/"/g'
        ```

        Nothing is lost either way: the same reason is on the node, in the two
        forms below, after the command has scrolled away.

=== "From the node"
    The same reason is recorded as an Event against the `Node`, so it turns up in
    `kubectl describe node` and in whatever already collects events from the
    cluster:

    ```text title="$ kubectl describe node worker-2"
    Events:
      Type     Reason     Age   From                Message
      ----     ------     ----  ----                -------
      Warning  JobFailed  2m    kata-deploy-job-dispatcher  its job kata-deploy-install-worker-2
                                failed: load-kernel-modules exited 1: this node has no
                                usable virtualization backend
    ```

    Listing them directly needs the `default` namespace rather than the release's:
    a `Node` has no namespace, and that is the only one the API server accepts an
    Event about a cluster-scoped object in — the same place the kubelet's own
    node events go.

    ```sh
    kubectl get events -n default --field-selector involvedObject.kind=Node
    ```

=== "Afterwards"
    Events expire, so the result is also written to the node itself and stays
    until the next rollout overwrites it:

    ```sh title="$ kubectl get nodes -l kata-deploy-job-dispatcher/result=failed"
    NAME       STATUS   ROLES    AGE   VERSION
    worker-2   Ready    <none>   9d    v1.34.1
    ```

    ```sh title="$ kubectl get node worker-2 -o jsonpath='{.metadata.annotations}'"
    kata-deploy-job-dispatcher/error:       its job kata-deploy-install-worker-2 failed: ...
    kata-deploy-job-dispatcher/finished-at: 2026-09-03T18:22:41Z
    ```

What makes any of that possible is that each stage writes why it is giving up to
`/dev/termination-log`, which lands in the pod's status rather than only in its
log. The dispatcher reads it the moment a Job is judged, and passes it on. The
dispatcher's own pod keeps the same summary in its status as a fallback for the
three places above: a dispatcher killed before it could report anything is
explained by `kubectl describe pod` alone.

!!! tip "A node that is stuck, rather than failed"
    Nothing marks a Job whose pod cannot be scheduled or cannot pull its image:
    it stays *running* until `job.activeDeadlineSeconds` expires, which is an
    hour on the defaults. Two minutes in, and every five after that, the
    dispatcher says what the wait is for — as a log line and as a
    `Warning`/`JobPending` Event on the node.

!!! note "Where the log still helps"
    The summary carries one line per node. The per-node Job's own log carries
    everything the stage printed on the way there, and `job.ttlSecondsAfterFinished`
    (10 minutes by default) is how long you have to read it. Raise it on a cluster
    where nobody is watching the rollout live.

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

On upgrade, `job` mode also compares that desired set with the nodes carrying
this installation's marker. Nodes in `owned - desired` run the cleanup pipeline
before the desired nodes are installed, so narrowing or changing a selector does
not leave the old nodes labelled and configured forever.

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

Finding one eligible node is not the end of the wait either. Eligibility arrives
per node, so the dispatcher waits until the set stays unchanged for
`job.nodeSettleSeconds` (default `15`) or the overall wait expires. A single
unchanged poll is not enough: the next node may be labelled a second later.

```yaml title="values.yaml"
job:
  # Give a slow-labelling cluster longer (see the timeout warning below)...
  waitForNodesSeconds: 300
  # Require a 30-second quiet period after the last newly eligible node.
  nodeSettleSeconds: 30
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

#### Bounding a node that never finishes

The dispatcher waits for every node it dispatched to, so one host wedged on
something it cannot finish — a CRI restart that never returns, a hung mount —
would otherwise hold up the whole rollout until Helm's own timeout fired. Two
knobs bound that:

```yaml title="values.yaml"
job:
  # Kubernetes fails a per-node Job that runs longer than this, and the
  # dispatcher reports that node as failed and carries on with the others.
  activeDeadlineSeconds: 3600
  # How long a finished per-node Job is kept. The dispatcher reads each Job's
  # result by polling it, so a Job collected before its next poll leaves its node
  # with no result at all - which reads as a failure. The chart refuses anything
  # below 60.
  ttlSecondsAfterFinished: 600
```

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
deliberate. A node is claimed with `katacontainers.io/kata-runtime=false` before
anything is written to it — by the dispatcher in `job` mode, before it even creates
that node's Job — and only promoted to `"true"` once the node is kata-capable.
Uninstall therefore also reaches nodes where the install failed part way through —
the ones most likely to have artifacts or CRI configuration left on them. Nothing
schedules onto a claimed node in the meantime, since the RuntimeClasses select the
exact value `"true"`. For the same reason an uninstall demotes the label to
`"false"` rather than removing it, and removes it only once that node's cleanup Job
has succeeded: a cleanup that fails is still a node the next `helm uninstall` can
find. Where several installations share a node, neither the demotion nor the removal
happens while another one is still holding it — see
[How installations keep out of each other's way on a node](#how-installations-keep-out-of-each-others-way-on-a-node).

In `job` mode claiming is a precondition for dispatch: if the labels cannot be
written, that node fails before any host mutation begins. This keeps the default
cleanup selector complete even when the API server rejects or times out a patch.

Cleanup pods also tolerate **every** taint, unlike the install's, which carry the
`tolerations` you configured. Where an install may run is your decision; where an
uninstall must run is decided by what is already on the node. A `NoExecute` taint
added since the install would otherwise evict the cleanup pod from a node that has
Kata on it, and nothing would come back for it.

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

Each is published as a release asset and `-f` takes a URL, so helm fetches the
file itself. Replace `VERSION` in both the flag and the URL, or use
`/releases/latest/download/<file>` for the newest release.

!!! warning "Releases up to and including 4.1.0"
    Those releases do not carry the presets as assets. Fetch the desired file from
    its tag instead, replacing `<file>` with the preset filename:
    `https://raw.githubusercontent.com/kata-containers/kata-containers/refs/tags/VERSION/tools/packaging/kata-deploy/helm-chart/kata-deploy/<file>`

### [`try-kata-tee.values.yaml`](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-tee.values.yaml)

This file enables only the TEE (Trusted Execution Environment) shims for confidential computing:

```sh
helm install kata-deploy oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy \
  --version VERSION \
  -f https://github.com/kata-containers/kata-containers/releases/download/VERSION/try-kata-tee.values.yaml
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
  -f https://github.com/kata-containers/kata-containers/releases/download/VERSION/try-kata-nvidia-cpu.values.yaml
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
  -f https://github.com/kata-containers/kata-containers/releases/download/VERSION/try-kata-nvidia-gpu.values.yaml
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

#### How installations keep out of each other's way on a node

One thing is deliberately *not* suffixed: the node label
`katacontainers.io/kata-runtime`, which every installation's RuntimeClasses select.
It says "this node can run Kata", not "this installation is here", so it cannot be
used to work out whether removing it is safe.

Each installation therefore also marks the nodes it holds with a label of its own,
named after its suffix — `kata-deploy.katacontainers.io/cicd` for the example above,
and `kata-deploy.katacontainers.io/default` for an installation that sets no suffix.
The value follows the shared label's: `false` while the installation is being put in
place, `true` once the node can run its workloads.

```sh
$ kubectl get node worker-1 -o jsonpath='{.metadata.labels}' | tr ',' '\n' | grep kata
"katacontainers.io/kata-runtime":"true"
"kata-deploy.katacontainers.io/default":"true"
"kata-deploy.katacontainers.io/cicd":"true"
```

Every installation's RuntimeClasses select **both** labels — the shared one and
its own mark, including `kata-deploy.katacontainers.io/default` when no suffix was
set. The default installation can still share a node with a suffixed release, so
it needs the same gate. Removing one mark stops that installation's workloads
being sent there immediately, even when the shared label stays for another one.

An uninstall removes its own mark from every node it reaches, and the shared label
only from the nodes where no other mark is left. So uninstalling `kata-deploy-cicd`
above leaves `worker-1` running Kata for the other installation, while a node that
only ever had `cicd` on it loses the label and stops being a Kata node. Cleanup
still restarts the shared CRI runtime after removing this installation's
configuration: otherwise the live runtime could keep advertising a handler whose
binary has just been deleted. The other installation's configuration remains and
is reloaded by that restart. Host-mutating stages from different releases take
the same node-local lock, so concurrent installs or uninstalls cannot race their
configuration edits and restarts.

A mark reading `false` does not count as another installation serving Kata: if the
last `true` mark goes and only a half-finished installation is left, the shared
label is set back to `false` rather than removed. Nothing is scheduled on the
strength of it, and the installation that is still working on the node keeps a
label its own uninstall can find.

!!! warning "Upgrade every installation before uninstalling any"

    Nodes installed by a version that did not write marks carry none, and an
    uninstall cannot attribute such a node to one release. Run `helm upgrade` on
    every installation without changing its node selection before uninstalling or
    narrowing one of them, so each release has first marked the nodes it owns.

`env.multiInstallSuffix` is immutable for the life of a Helm release. The chart
stores the first value under a suffix-independent ConfigMap name and rejects an
upgrade that changes it: the install directory, handlers, resource names, and
node marker all derive from the suffix, so changing it in place would create a
second installation while forgetting how to remove the first.

For containerd, a suffixed installation also requires drop-in configuration
support. Older whole-file configuration has one shared backup; uninstalling one
release would restore a file that predates every release and erase the surviving
handlers. Installation therefore fails before modifying CRI configuration when
that unsafe combination is detected.

With whole-file configuration, uninstall restores the backup taken at install
time. A node whose containerd had no configuration file at all gets one written
for it, and uninstall deletes that file again. Any other whole-file configuration
is left untouched, with a warning in the log: without a backup or a record of
having written the file, kata-deploy cannot tell its own configuration from an
administrator's.

On the first upgrade from a chart version that predates that state ConfigMap, the
chart verifies the previous identity from the existing mode-specific resource
whose name derives from the suffix. If it cannot find one, the upgrade stops and
the error names the ConfigMap data that can be seeded explicitly; guessing would
be the unsafe choice.

The same state makes `deploymentMode` immutable and rejects values other than
`daemonset` and `job`. An in-place mode switch can strand nodes owned by the old
controller, and recording a new mode before its hook succeeds can also make a
rollback reject the previous mode. Uninstall the existing mode cleanly before
installing the other one.

In `job` mode this means an uninstall may visit a node that only ever belonged to
another installation — the default cleanup selection is the shared label, which is
by definition not yours alone — and find nothing of its own to remove. That is
deliberately the safe direction: it reaches every node it might have touched. If you
would rather it visited only its own nodes, select on its mark:

```yaml title="values.yaml"
job:
  cleanup:
    nodeAffinity:
      requiredDuringSchedulingIgnoredDuringExecution:
        nodeSelectorTerms:
          - matchExpressions:
              - key: kata-deploy.katacontainers.io/cicd
                operator: Exists
```

!!! note "Both deployment modes keep the same books"

    The marks are the same labels in either mode — written by the dispatcher in
    `job` mode and by the pod on the node in `daemonset` mode — so an uninstall in
    one mode does see an installation running in the other. Mixing modes across
    installations is not a combination we test, though: give every installation the
    same `deploymentMode`.

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
is registered. `true` and `false` decide outright.

`false` turns off **both** halves: no rule, and no confidential `RuntimeClass` asks
for a TEE key. It is the escape hatch for a cluster that wants no part of this — not
the way to keep the requests while managing the rule elsewhere. If you create an
equivalent rule yourself, leave this at `auto` (or set it to `true`) so the requests
are still rendered, and delete the chart's rule if it duplicates yours.

!!! note "Naming, and a rule you may have to delete by hand"
    The chart's rule is named `kata-tee-keys`, suffixed with
    `env.multiInstallSuffix` when that is set. A rule called `amd64-tee-keys`
    belongs to a release that has not been upgraded yet, and can be deleted once
    every release has been.

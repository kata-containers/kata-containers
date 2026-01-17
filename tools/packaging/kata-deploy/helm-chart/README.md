# Kata Containers Deploy – Helm Chart

A Helm chart that installs the kata-deploy DaemonSet and its helper assets,
enabling Kata Containers runtimes on your Kubernetes, K3s, RKE2, or K0s cluster.

## TL;DR

```sh
# Install directly from the official ghcr.io OCI regitry
# update the VERSION X.YY.Z to your needs or just use the latest

export VERSION=$(curl -sSL https://api.github.com/repos/kata-containers/kata-containers/releases/latest | jq .tag_name | tr -d '"')
export CHART="oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy"

$ helm install kata-deploy "${CHART}" --version "${VERSION}"

# See everything you can configure
$ helm show values "${CHART}" --version "${VERSION}"
```

## Prerequisites

- **Kubernetes ≥ v1.22** – v1.22 is the first release where the CRI v1 API
  became the default and `RuntimeClass` left alpha.  The chart depends on those
  stable interfaces; earlier clusters need `feature‑gates` or CRI shims that are
  out of scope.

- **Kata Release 3.12** - v3.12.0 introduced publishing the helm-chart on the
  release page for easier consumption, since v3.8.0 we shipped the helm-chart
  via source code in the kata-containers `Github` repository.

- CRI‑compatible runtime (containerd or CRI‑O). If one wants to use the
  `multiInstallSuffix` feature one needs at least **containerd-2.0** which
  supports drop-in config files

- Nodes must allow loading kernel modules and installing Kata artifacts (the
  chart runs privileged containers to do so)

## Installing Helm

If Helm is not yet on your workstation or CI runner, install Helm v3 (v3.9 or
newer recommended):

```sh
# Quick one‑liner (Linux/macOS)
$ curl https://raw.githubusercontent.com/helm/helm/main/scripts/get-helm-3 | bash

# Or via your package manager
$ sudo apt-get install helm        # Debian/Ubuntu
$ brew install helm                # Homebrew on macOS / Linuxbrew
```

Verify the installation:

```sh
$ helm version
```

## Installing the Chart

Before attempting installing the chart one may first consult the table below
[Configuration Reference](#configuration-reference) for all the default values.
Some default values may not fit all use-cases so update as needed. A prime example
may be the `k8sDistribution` which per default is set to `k8s`.

To see which chart versions are available either use the CLI

```sh
$ helm show chart oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy
```

or visit
[kata-deploy-charts](https://github.com/orgs/kata-containers/packages/container/package/kata-deploy-charts%2Fkata-deploy)

If one wants to wait until the Helm chart has deployed every object in the chart
one can use `--wait --timeout 10m --atomic`. If the timeout expires or anything
fails, Helm rolls the release back to its previous state.

```sh
$ helm install kata-deploy \        # release name
  --namespace kube-system \         # recommended namespace
  --wait --timeout 10m --atomic \
  "${CHART}" --version  "${VERSION}"
```

If one does not want to wait for the object via Helm or one wants rather use
`kubectl` use Helm like this:

```sh
$ helm install kata-deploy \                         # release name
  --namespace kube-system \                          # recommended namespace
  "${CHART}" --version  "${VERSION}"
```

## Updating Settings

Forgot to enable an option? Re‑use the values already on the cluster and only
mutate what you need:

```sh
# List existing releases
$ helm ls -A

# Upgrade in‑place, keeping everything else the same
$ helm upgrade kata-deploy -n kube-system \
  --reuse-values \
  --set env.defaultShim=qemu-runtime-rs \
  "${CHART}" --version  "${VERSION}"
```

## Uninstalling

```sh
$ helm uninstall kata-deploy -n kube-system
```

## Configuration Reference

All values can be overridden with --set key=value or a custom `-f myvalues.yaml`.

| Key | Description | Default |
|-----|-------------|---------|
| `imagePullPolicy` | Set the DaemonSet pull policy | `Always` |
| `imagePullSecrets` | Enable pulling from a private registry via pull secret | `""` |
| `image.reference` | Fully qualified image reference | `quay.io/kata-containers/kata-deploy` |
| `image.tag` | Tag of the image reference | `""` |
| `k8sDistribution` | Set the k8s distribution to use: `k8s`, `k0s`, `k3s`, `rke2`, `microk8s` | `k8s` |
| `nodeSelector` | Node labels for pod assignment. Allows restricting deployment to specific nodes | `{}` |
| `runtimeClasses.enabled` | Enable Helm-managed `runtimeClass` creation (recommended) | `true` |
| `runtimeClasses.createDefault` | Create a default `runtimeClass` alias for the default shim | `false` |
| `runtimeClasses.defaultName` | Name for the default `runtimeClass` | `kata` |
| `env.debug` | Enable debugging in the `configuration.toml` | `false` |
| `env.shims` | List of shims to deploy | `clh cloud-hypervisor dragonball fc qemu qemu-coco-dev qemu-coco-dev-runtime-rs qemu-runtime-rs qemu-se-runtime-rs qemu-snp qemu-tdx stratovirt qemu-nvidia-gpu qemu-nvidia-gpu-snp qemu-nvidia-gpu-tdx qemu-cca` |
| `env.shims_x86_64` | List of shims to deploy for x86_64 (if set, overrides `shims`) | `""` |
| `env.shims_aarch64` | List of shims to deploy for aarch64 (if set, overrides `shims`) | `""` |
| `env.shims_s390x` | List of shims to deploy for s390x (if set, overrides `shims`) | `""` |
| `env.shims_ppc64le` | List of shims to deploy for ppc64le (if set, overrides `shims`) | `""` |
| `env.defaultShim` | The default shim to use if none specified | `qemu` |
| `env.defaultShim_x86_64` | The default shim to use if none specified for x86_64 (if set, overrides `defaultShim`) | `""` |
| `env.defaultShim_aarch64` | The default shim to use if none specified for aarch64 (if set, overrides `defaultShim`) | `""` |
| `env.defaultShim_s390x` | The default shim to use if none specified for s390x (if set, overrides `defaultShim`) | `""` |
| `env.defaultShim_ppc64le` | The default shim to use if none specified for ppc64le (if set, overrides `defaultShim`) | `""` |
| `env.createRuntimeClasses` | **DEPRECATED** - Use `runtimeClasses.enabled` instead. Script-based `runtimeClass` creation | `false` |
| `env.createDefaultRuntimeClass` | **DEPRECATED** - Use `runtimeClasses.createDefault` instead | `false` |
| `env.allowedHypervisorAnnotations` | Enable the provided annotations to be enabled when launching a Container or Pod, per default the annotations are disabled | `""` |
| `env.snapshotterHandlerMapping` | Provide the snapshotter handler for each shim | `""` |
| `env.snapshotterHandlerMapping_x86_64` | Provide the snapshotter handler for each shim for x86_64 (if set, overrides `snapshotterHandlerMapping`) | `""` |
| `env.snapshotterHandlerMapping_aarch64` | Provide the snapshotter handler for each shim for aarch64 (if set, overrides `snapshotterHandlerMapping`) | `""` |
| `env.snapshotterHandlerMapping_s390x` | Provide the snapshotter handler for each shim for s390x (if set, overrides `snapshotterHandlerMapping`) | `""` |
| `env.snapshotterHandlerMapping_ppc64le` | Provide the snapshotter handler for each shim for ppc64le (if set, overrides `snapshotterHandlerMapping`) | `""` |
| `evn.agentHttpsProxy` | HTTPS_PROXY=... | `""` |
| `env.agentHttpProxy` |  specifies a list of addresses that should bypass a configured proxy server | `""` |
| `env.pullTypeMapping` | Type of container image pulling, examples are guest-pull or default | `""` |
| `env.pullTypeMapping_x86_64` | Type of container image pulling for x86_64 (if set, overrides `pullTypeMapping`) | `""` |
| `env.pullTypeMapping_aarch64` | Type of container image pulling for aarch64 (if set, overrides `pullTypeMapping`) | `""` |
| `env.pullTypeMapping_s390x` | Type of container image pulling for s390x (if set, overrides `pullTypeMapping`) | `""` |
| `env.pullTypeMapping_ppc64le` | Type of container image pulling for ppc64le (if set, overrides `pullTypeMapping`) | `""` |
| `env.installationPrefix` | Prefix where to install the Kata artifacts | `/opt/kata` |
| `env.hostOS` | Provide host-OS setting, e.g. `cbl-mariner` to do additional configurations | `""` |
| `env.multiInstallSuffix` | Enable multiple Kata installation on the same node with suffix e.g. `/opt/kata-PR12232` | `""` |
| `env._experimentalSetupSnapshotter` | Deploys (nydus) and/or sets up (erofs, nydus) the snapshotter(s) specified as the value (supports multiple snapshotters, separated by commas; e.g., `nydus,erofs`) | `""` |
| `env._experimentalForceGuestPull` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value (supports multiple shims, separated by commas; e.g., `qemu-tdx,qemu-snp`) | `""` |
| `env._experimentalForceGuestPull_x86_64` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value for x86_64 (if set, overrides `_experimentalForceGuestPull`) | `""` |
| `env._experimentalForceGuestPull_aarch64` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value for aarch64 (if set, overrides `_experimentalForceGuestPull`) | `""` |
| `env._experimentalForceGuestPull_s390x` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value for s390x (if set, overrides `_experimentalForceGuestPull`) | `""` |
| `env._experimentalForceGuestPull_ppc64le` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value for ppc64le (if set, overrides `_experimentalForceGuestPull`) | `""` |

## Structured Configuration

**NEW**: Starting with Kata Containers v3.23.0, a new structured configuration format is available for configuring shims. This provides better type safety, clearer organization, and per-shim configuration options.

### Migration from Legacy Format

The legacy `env.*` configuration format is **deprecated** and will be removed in 2 releases. Users are encouraged to migrate to the new structured format.

**Deprecated fields** (will be removed in 2 releases):
- `env.shims`, `env.shims_x86_64`, `env.shims_aarch64`, `env.shims_s390x`, `env.shims_ppc64le`
- `env.defaultShim`, `env.defaultShim_x86_64`, `env.defaultShim_aarch64`, `env.defaultShim_s390x`, `env.defaultShim_ppc64le`
- `env.allowedHypervisorAnnotations`
- `env.snapshotterHandlerMapping`, `env.snapshotterHandlerMapping_x86_64`, etc.
- `env.pullTypeMapping`, `env.pullTypeMapping_x86_64`, etc.
- `env.agentHttpsProxy`, `env.agentNoProxy`
- `env._experimentalSetupSnapshotter`
- `env._experimentalForceGuestPull`, `env._experimentalForceGuestPull_x86_64`, etc.
- `env.debug`

### New Structured Format

The new format uses a `shims` section where each shim can be configured individually:

```yaml
# Enable debug mode globally
debug: false

# Configure snapshotter setup
snapshotter:
  setup: []  # ["nydus", "erofs"] or []

# Configure shims
shims:
  qemu:
    enabled: true
    supportedArches:
      - amd64
      - arm64
      - s390x
      - ppc64le
    allowedHypervisorAnnotations: []
    containerd:
      snapshotter: ""
  qemu-snp:
    enabled: true
    supportedArches:
      - amd64
    allowedHypervisorAnnotations: []
    containerd:
      snapshotter: nydus
      forceGuestPull: false
    crio:
      guestPull: true
    agent:
      httpsProxy: ""
      noProxy: ""

# Default shim per architecture
defaultShim:
  amd64: qemu
  arm64: qemu
  s390x: qemu
  ppc64le: qemu
```

### Key Benefits

1. **Per-shim configuration**: Each shim can have its own settings for snapshotter, guest pull, agent proxy, etc.
2. **Architecture-aware**: Shims declare which architectures they support
3. **Type safety**: Structured format reduces configuration errors
4. **Easy to use**: All shims are enabled by default in `values.yaml`, so you can use the chart directly without modification

### Example: Enable `qemu` shim with new format

```yaml
shims:
  qemu:
    enabled: true
    supportedArches:
      - amd64
      - arm64

defaultShim:
  amd64: qemu
  arm64: qemu
```

### Backward Compatibility

The chart maintains full backward compatibility with the legacy `env.*` format. If legacy values are set, they take precedence over the new structured format. This allows for gradual migration.

### Default Configuration

The default `values.yaml` file has **all shims enabled by default**, making it easy to use the chart directly without modification:

```sh
helm install kata-deploy oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy \
  --version VERSION
```

This includes all available Kata Containers shims:
- Standard shims: `qemu`, `qemu-runtime-rs`, `clh`, `cloud-hypervisor`, `dragonball`, `fc`
- TEE shims: `qemu-snp`, `qemu-tdx`, `qemu-se`, `qemu-se-runtime-rs`, `qemu-cca`, `qemu-coco-dev`, `qemu-coco-dev-runtime-rs`
- NVIDIA GPU shims: `qemu-nvidia-gpu`, `qemu-nvidia-gpu-snp`, `qemu-nvidia-gpu-tdx`
- Remote shims: `remote` (for `peer-pods`/`cloud-api-adaptor`, disabled by default)

To enable only specific shims, you can override the configuration:

```yaml
# Custom values file - enable only qemu shim
shims:
  qemu:
    enabled: true
  clh:
    enabled: false
  cloud-hypervisor:
    enabled: false
  # ... disable other shims as needed
```

### Example Values Files

For convenience, we also provide example values files that demonstrate specific use cases:

#### `try-kata-tee.values.yaml` - Trusted Execution Environment Shims

This file enables only the TEE (Trusted Execution Environment) shims for confidential computing:

```sh
helm install kata-deploy oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy \
  --version VERSION \
  -f try-kata-tee.values.yaml
```

Includes:
- `qemu-snp` - AMD SEV-SNP (amd64)
- `qemu-tdx` - Intel TDX (amd64)
- `qemu-se` - IBM Secure Execution (s390x)
- `qemu-se-runtime-rs` - IBM Secure Execution Rust runtime (s390x)
- `qemu-cca` - Arm Confidential Compute Architecture (arm64)
- `qemu-coco-dev` - Confidential Containers development (amd64, s390x)
- `qemu-coco-dev-runtime-rs` - Confidential Containers development Rust runtime (amd64, s390x)

#### `try-kata-nvidia-gpu.values.yaml` - NVIDIA GPU Shims

This file enables only the NVIDIA GPU-enabled shims:

```sh
helm install kata-deploy oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy \
  --version VERSION \
  -f try-kata-nvidia-gpu.values.yaml
```

Includes:
- `qemu-nvidia-gpu` - Standard NVIDIA GPU support (amd64, arm64)
- `qemu-nvidia-gpu-snp` - NVIDIA GPU with AMD SEV-SNP (amd64)
- `qemu-nvidia-gpu-tdx` - NVIDIA GPU with Intel TDX (amd64)

**Note**: These example files are located in the chart directory. When installing from the OCI registry, you'll need to download them separately or clone the repository to access them.

## `RuntimeClass` Management

**NEW**: Starting with Kata Containers v3.23.0, `runtimeClasses` are managed by
         Helm by default, providing better lifecycle management and integration.

### Features:
- **Automatic Creation**: `runtimeClasses` are automatically created for all configured shims
- **Lifecycle Management**: Helm manages creation, updates, and deletion of `runtimeClasses`

### Configuration:
```yaml
runtimeClasses:
  enabled: true                  # Enable Helm-managed `runtimeClasses` (default)
  createDefault: false           # Create a default "kata" `runtimeClass`
  defaultName: "kata"            # Name for the default `runtimeClass`
```

When `runtimeClasses.enabled: true` (default), the Helm chart creates
`runtimeClass` resources for all enabled shims (either from the new structured
`shims` configuration or from the legacy `env.shims` format).

The kata-deploy script will no longer create `runtimeClasses`
(`env.createRuntimeClasses` defaults to `"false"`).

## Example: only `qemu` shim and debug enabled

Since all shims are enabled by default, you need to disable the ones you don't want:

```sh
# Using --set flags (disable all except qemu)
$ helm install kata-deploy \
  --set shims.clh.enabled=false \
  --set shims.cloud-hypervisor.enabled=false \
  --set shims.dragonball.enabled=false \
  --set shims.fc.enabled=false \
  --set shims.qemu-runtime-rs.enabled=false \
  --set shims.qemu-nvidia-gpu.enabled=false \
  --set shims.qemu-nvidia-gpu-snp.enabled=false \
  --set shims.qemu-nvidia-gpu-tdx.enabled=false \
  --set shims.qemu-snp.enabled=false \
  --set shims.qemu-tdx.enabled=false \
  --set shims.qemu-se.enabled=false \
  --set shims.qemu-se-runtime-rs.enabled=false \
  --set shims.qemu-cca.enabled=false \
  --set shims.qemu-coco-dev.enabled=false \
  --set shims.qemu-coco-dev-runtime-rs.enabled=false \
  --set debug=true \
  "${CHART}" --version  "${VERSION}"
```

Or use a custom values file:

```yaml
# custom-values.yaml
debug: true
shims:
  qemu:
    enabled: true
  clh:
    enabled: false
  cloud-hypervisor:
    enabled: false
  dragonball:
    enabled: false
  fc:
    enabled: false
  qemu-runtime-rs:
    enabled: false
  qemu-nvidia-gpu:
    enabled: false
  qemu-nvidia-gpu-snp:
    enabled: false
  qemu-nvidia-gpu-tdx:
    enabled: false
  qemu-snp:
    enabled: false
  qemu-tdx:
    enabled: false
  qemu-se:
    enabled: false
  qemu-se-runtime-rs:
    enabled: false
  qemu-cca:
    enabled: false
  qemu-coco-dev:
    enabled: false
  qemu-coco-dev-runtime-rs:
    enabled: false
  remote:
    enabled: false
```

```sh
$ helm install kata-deploy \
  -f custom-values.yaml \
  "${CHART}" --version  "${VERSION}"
```

## Example: Deploy only to specific nodes using `nodeSelector`

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
# values.yaml
nodeSelector:
  kata-containers: "enabled"
  node-type: "worker"
```

```sh
$ helm install kata-deploy -f values.yaml "${CHART}" --version "${VERSION}"
```

## Example: Multiple Kata installations on the same node

For debugging, testing and other use-case it is possible to deploy multiple
versions of Kata on the very same node. All the needed artifacts are getting the
`mulitInstallSuffix` appended to distinguish each installation. **BEWARE** that one
needs at least **containerd-2.0** since this version has drop-in conf support
which is a prerequisite for the `mulitInstallSuffix` to work properly.

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
kata-cloud-hypervisor-cicd      kata-cloud-hypervisor-cicd      77s
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

## Custom Runtimes

Starting with Kata Containers v3.26.0, you can bring your own `RuntimeClass`
definitions with custom `podOverhead` values using a base config + drop-in approach.

This is useful when you need:
- Different memory/CPU overhead values for specific workloads
- Custom VM configurations (different default memory, vCPUs, etc.)
- Multiple runtime variants on the same cluster
- Custom `CoCo`/guest-pull configurations

### How Custom Runtimes Work

Custom runtimes leverage Kata's existing `config.d` drop-in mechanism:

1. You specify a **`baseConfig`** - an existing Kata configuration to use as the base
2. You optionally provide a **`dropIn`** - TOML overrides that are applied on top
3. You define a **`RuntimeClass`** - with your custom handler name and `podOverhead`

The shim binary (Go vs Rust runtime) is automatically determined from the `baseConfig`.

### Important: Configuration Inheritance

The base config is copied **after** kata-deploy has applied its modifications based on
Helm values (debug mode, proxy settings, hypervisor annotations). This means your custom
runtime inherits these settings from the base config.

If you need different settings, override them in your `dropIn` content.

### Usage

Create a values file with your custom runtimes:

```yaml
# custom-runtimes.values.yaml
customRuntimes:
  enabled: true
  runtimes:
    my-gpu-runtime:
      baseConfig: "qemu-nvidia-gpu"   # Required: existing config as base
      dropIn: |                       # Optional: Override specific settings
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
```

Deploy with:

```sh
helm install kata-deploy "${CHART}" --version "${VERSION}" \
  -f custom-runtimes.values.yaml
```

### Available Base Configs

Use any existing Kata configuration as your `baseConfig` value, for example:
`qemu`, `qemu-nvidia-gpu`, `qemu-snp`, `qemu-tdx`, `cloud-hypervisor`, `fc`, etc.

The correct shim binary is automatically selected based on the `baseConfig`.

### CRI-Specific Configuration

For `CoCo`/guest-pull scenarios, you can configure CRI-specific settings:

```yaml
customRuntimes:
  enabled: true
  runtimes:
    my-coco-runtime:
      baseConfig: "qemu-snp"
      dropIn: |
        [hypervisor.qemu]
        default_memory = 2048
      runtimeClass: |
        kind: RuntimeClass
        apiVersion: node.k8s.io/v1
        metadata:
          name: kata-my-coco-runtime
          labels:
            app.kubernetes.io/managed-by: kata-deploy
        handler: kata-my-coco-runtime
        overhead:
          podFixed:
            memory: "1Gi"
            cpu: "500m"
        scheduling:
          nodeSelector:
            katacontainers.io/kata-runtime: "true"
      containerd:
        snapshotter: "nydus"  # Configure nydus snapshotter
      crio:
        pullType: "guest-pull"  # Enable runtime_pull_image = true
```

| Field | Description | Values |
|-------|-------------|--------|
| `baseConfig` | Base configuration to use (required) | See "Available Base Configs" above |
| `dropIn` | TOML overrides applied via `config.d` mechanism (optional) | Any valid TOML configuration |
| `containerd.snapshotter` | Configure containerd snapshotter | `nydus`, `erofs`, or empty for default |
| `crio.pullType` | Configure CRI-O image pulling | `guest-pull` or empty for default |

### Using Custom Runtimes in Pods

Reference the custom `RuntimeClass` in your pod spec:

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: my-gpu-workload
spec:
  runtimeClassName: kata-my-gpu-runtime
  containers:
  - name: app
    image: nvidia/cuda:12.0-base
```

### What Gets Created

For each custom runtime, kata-deploy creates an isolated directory structure:

```
/opt/kata/share/defaults/kata-containers/custom-runtimes/
└── kata-my-gpu-runtime/
    ├── configuration-qemu-nvidia-gpu.toml  # Copy of modified base
    └── config.d/
        └── 50-overrides.toml               # Your drop-in (if provided)
```

| Resource | Description |
|----------|-------------|
| `RuntimeClass` | Kubernetes resource with your handler and `podOverhead` |
| Handler | Registered in containerd/CRI-O pointing to the isolated config |
| Base Config | Copy of the base config (with kata-deploy modifications applied) |
| Drop-in | Your overrides in `config.d/` (Kata merges these automatically) |

### Cleanup

When you run `helm uninstall`:
- `RuntimeClasses` are automatically deleted by Helm
- Custom handlers are removed from containerd/CRI-O config
- Custom runtime directories are deleted from host nodes

### Tips

1. **Start simple**: Use `baseConfig` without `dropIn` first, then add overrides as needed
2. **Check inheritance**: Remember your custom runtime inherits debug/proxy/annotation settings from the base
3. **Handler naming**: Use descriptive names like `kata-high-memory` or `kata-coco-custom`
4. **Drop-in format**: Only include the sections you want to override in `dropIn`

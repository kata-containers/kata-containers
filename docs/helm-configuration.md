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
      - arm64
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
- `qemu-cca` - Arm Confidential Compute Architecture (arm64)
- `qemu-coco-dev` - Confidential Containers development (amd64, s390x)
- `qemu-coco-dev-runtime-rs` - Confidential Containers development Rust runtime (amd64, s390x)

### [`try-kata-nvidia-gpu.values.yaml`](https://github.com/kata-containers/kata-containers/blob/main/tools/packaging/kata-deploy/helm-chart/kata-deploy/try-kata-nvidia-gpu.values.yaml)

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

## Customizing Configuration with Drop-in Files

When kata-deploy installs Kata Containers, the base configuration files should not
be modified directly. Instead, use drop-in configuration files to customize
settings. This approach ensures your customizations survive kata-deploy upgrades.

### How Drop-in Files Work

The Kata runtime reads the base configuration file and then applies any `.toml`
files found in the `config.d/` directory alongside it. Files are processed in
alphabetical order, with later files overriding earlier settings.

### Creating Custom Drop-in Files

To add custom settings, create a `.toml` file in the appropriate `config.d/`
directory. Use a numeric prefix to control the order of application.

**Reserved prefixes** (used by kata-deploy):

- `10-*`: Core kata-deploy settings
- `20-*`: Debug settings
- `30-*`: Kernel parameters

**Recommended prefixes for custom settings**: `50-89`

### Examples

#### Adding Custom Kernel Parameters

```bash
# SSH into the node or use kubectl exec
sudo mkdir -p /opt/kata/share/defaults/kata-containers/runtimes/qemu/config.d/
sudo cat > /opt/kata/share/defaults/kata-containers/runtimes/qemu/config.d/50-custom.toml << 'EOF'
[hypervisor.qemu]
kernel_params = "my_param=value"
EOF
```

#### Changing Default Memory Size

```bash
sudo cat > /opt/kata/share/defaults/kata-containers/runtimes/qemu/config.d/50-memory.toml << 'EOF'
[hypervisor.qemu]
default_memory = 4096
EOF
```

## Legacy Parameters

The structured configuration format described above is available as of Kata Containers v3.23.0. The legacy format is described below.

| Key | Description | Default |
|-----|-------------|---------|
| `imagePullPolicy` | Set the DaemonSet pull policy | `Always` |
| `imagePullSecrets` | Enable pulling from a private registry via pull secret | `""` |
| `image.reference` | Fully qualified image reference (for digest pinning use the full image e.g. `…@sha256:...`; tag is ignored) | `quay.io/kata-containers/kata-deploy` |
| `image.tag` | Tag of the image reference (defaults to chart `AppVersion` when empty) | `""` |
| `kubectlImage.reference` | Fully qualified `kubectl` image reference (for digest pinning use the full image e.g. `…@sha256:...` and leave `kubectlImage.tag` empty) | `quay.io/kata-containers/kubectl` |
| `kubectlImage.tag` | Tag of the `kubectl` image reference | `latest` |
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
| `env._experimentalSetupSnapshotter` | Deploys (`nydus`) and/or sets up (`erofs`, `nydus`) the snapshotter(s) specified as the value (supports multiple snapshotters, separated by commas; e.g., `nydus,erofs`) | `""` |
| `env._experimentalForceGuestPull` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value (supports multiple shims, separated by commas; e.g., `qemu-tdx,qemu-snp`) | `""` |
| `env._experimentalForceGuestPull_x86_64` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value for x86_64 (if set, overrides `_experimentalForceGuestPull`) | `""` |
| `env._experimentalForceGuestPull_aarch64` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value for aarch64 (if set, overrides `_experimentalForceGuestPull`) | `""` |
| `env._experimentalForceGuestPull_s390x` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value for s390x (if set, overrides `_experimentalForceGuestPull`) | `""` |
| `env._experimentalForceGuestPull_ppc64le` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value for ppc64le (if set, overrides `_experimentalForceGuestPull`) | `""` |


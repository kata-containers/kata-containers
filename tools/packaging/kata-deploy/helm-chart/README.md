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
| `env.debug` | Enable debugging in the `configuration.toml` | `false` |
| `env.shims` | List of shims to deploy | `clh cloud-hypervisor dragonball fc qemu qemu-coco-dev qemu-runtime-rs qemu-se-runtime-rs qemu-snp qemu-tdx stratovirt qemu-nvidia-gpu qemu-nvidia-gpu-snp qemu-nvidia-gpu-tdx qemu-cca qemu-runtime-rs-coco-dev` |
| `env.defaultShim` | The default shim to use if none specified | `qemu` |
| `env.createRuntimeClasses` | Create the k8s `runtimeClasses` | `true` |
| `env.createDefaultRuntimeClass` | Create the default k8s `runtimeClass` | `false` |
| `env.allowedHypervisorAnnotations` | Enable the provided annotations to be enabled when launching a Container or Pod, per default the annotations are disabled | `""` |
| `env.snapshotterHandlerMapping` | Provide the snapshotter handler for each shim | `""` |
| `evn.agentHttpsProxy` | HTTPS_PROXY=... | `""` |
| `env.agentHttpProxy` |  specifies a list of addresses that should bypass a configured proxy server | `""` |
| `env.pullTypeMapping` | Type of container image pulling, examples are guest-pull or default | `""` |
| `env.installationPrefix` | Prefix where to install the Kata artifacts | `/opt/kata` |
| `env.hostOS` | Provide host-OS setting, e.g. `cbl-mariner` to do additional configurations | `""` |
| `env.multiInstallSuffix` | Enable multiple Kata installation on the same node with suffix e.g. `/opt/kata-PR12232` | `""` |
| `env._experimentalSetupSnapshotter` | Deploys (nydus) and/or sets up (erofs, nydus) the snapshotter(s) specified as the value (supports multiple snapshotters, separated by commas; e.g., `nydus,erofs`) | `""` |
| `env._experimentalForceGuestPull` | Enables `experimental_force_guest_pull` for the shim(s) specified as the value (supports multiple shims, separated by commas; e.g., `qemu-tdx,qemu-snp`) | `""` |

## Example: only `qemu` shim and debug enabled

```sh
$ helm install kata-deploy \
  --set env.shims="qemu" \
  --set env.debug=true \
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
  --set env.createRuntimeClasses=true \
  "${CHART}" --version  "${VERSION}"
```

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

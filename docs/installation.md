# Installation

## Helm Chart

[helm](https://helm.sh/docs/intro/install/) can be used to install templated kubernetes manifests.

### Prerequisites

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

### `helm install`

```sh
# Install directly from the official ghcr.io OCI regitry
# update the VERSION X.YY.Z to your needs or just use the latest

export VERSION=$(curl -sSL https://api.github.com/repos/kata-containers/kata-containers/releases/latest | jq .tag_name | tr -d '"')
export CHART="oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy"

$ helm install kata-deploy "${CHART}" --version "${VERSION}"

# See everything you can configure
$ helm show values "${CHART}" --version "${VERSION}"
```

This creates a new Runtime Class `kata-custom` that extends the `qemu`
configuration with your custom settings.

To see what versions of the chart are available:

```sh
$ helm show chart oci://ghcr.io/kata-containers/kata-deploy-charts/kata-deploy
```

### `helm uninstall`

```sh
$ helm uninstall kata-deploy -n kube-system
```

During uninstall, Helm will report that some resources were kept due to the
resource policy (`ServiceAccount`, `ClusterRole`, `ClusterRoleBinding`). This
is **normal**. A post-delete hook Job runs after uninstall and removes those
resources so no cluster-wide `RBAC` is left behind.

## Pre-built Release

Kata can also be installed using the pre-built releases: https://github.com/kata-containers/kata-containers/releases

This method does not have any facilities for artifact lifecycle management.
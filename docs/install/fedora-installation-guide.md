# Install Kata Containers on Fedora

1. Install the Kata Containers components with the following commands:

   ```bash
   $ source /etc/os-release
   $ ARCH=$(arch)
   $ BRANCH="${BRANCH:-master}"
   $ sudo dnf -y install dnf-plugins-core
   $ sudo -E dnf config-manager --add-repo "http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}/Fedora_${VERSION_ID}/home:katacontainers:releases:${ARCH}:${BRANCH}.repo"
   $ sudo -E dnf -y install kata-runtime kata-proxy kata-shim
   ```

2. Decide which container manager to use and select the corresponding link that follows:

   - [Docker](docker/fedora-docker-install.md)
   - [Kubernetes](../Developer-Guide.md#run-kata-containers-with-kubernetes)

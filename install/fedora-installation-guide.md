# Install Kata Containers on Fedora

1. Install the Kata Containers components with the following commands:

   ```bash
   $ source /etc/os-release
   $ ARCH=$(arch)
   $ sudo dnf -y install dnf-plugins-core
   $ sudo -E dnf config-manager --add-repo "http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/master/Fedora_${VERSION_ID}/home:katacontainers:releases:${ARCH}:master.repo"
   $ sudo -E dnf -y install kata-runtime kata-proxy kata-shim
   ```

2. Decide which container manager to use and select the corresponding link that follows:

   - [Docker](docker/fedora-docker-install.md)
   - [Kubernetes](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#run-kata-containers-with-kubernetes)

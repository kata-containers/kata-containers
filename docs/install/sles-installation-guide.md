# Install Kata Containers on SLES

1. Install the Kata Containers components with the following commands:

   ```bash
   $ ARCH=$(arch)
   $ BRANCH="${BRANCH:-master}"
   $ sudo -E zypper addrepo "http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}/SLE_15_SP1/home:katacontainers:releases:${ARCH}:${BRANCH}.repo"
   $ sudo -E zypper -n --no-gpg-checks install kata-runtime kata-proxy kata-shim
   ```

2. Decide which container manager to use and select the corresponding link that follows:

   - [Docker](docker/sles-docker-install.md)
   - [Kubernetes](../Developer-Guide.md#run-kata-containers-with-kubernetes)

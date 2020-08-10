# Install Kata Containers on RHEL

1. Install the Kata Containers components with the following commands:

   ```bash
   $ source /etc/os-release
   $ ARCH=$(arch)
   $ BRANCH="${BRANCH:-master}"
   $ sudo -E yum-config-manager --add-repo "http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}/RHEL_${VERSION_ID}/home:katacontainers:releases:${ARCH}:${BRANCH}.repo"
   $ sudo -E yum -y install kata-runtime kata-proxy kata-shim
   ```

2. Decide which container manager to use and select the corresponding link that follows:

   - [Docker](docker/rhel-docker-install.md)
   - [Kubernetes](../Developer-Guide.md#run-kata-containers-with-kubernetes)

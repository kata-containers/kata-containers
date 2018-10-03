# Install Kata Containers on openSUSE Leap

1. Install the Kata Containers components with the following commands:

   ```bash
   $ source /etc/os-release
   $ ARCH=$(arch)
   $ sudo -E zypper addrepo "http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/master/openSUSE_Leap_${VERSION_ID}/home:katacontainers:releases:${ARCH}:master.repo"
   $ sudo -E zypper -n --no-gpg-checks install kata-runtime kata-proxy kata-shim
   ```

2. Decide which container manager to use and select the corresponding link that follows:

   - [Docker](docker/opensuse-docker-install.md)
   - [Kubernetes](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#run-kata-containers-with-kubernetes)

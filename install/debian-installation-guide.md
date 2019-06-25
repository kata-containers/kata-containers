# Install Kata Containers on Debian

1. Install the Kata Containers components with the following commands:

   ```bash
   $ export DEBIAN_FRONTEND=noninteractive
   $ ARCH=$(arch)
   $ BRANCH="${BRANCH:-master}"
   $ source /etc/os-release
   $ [ "$ID" = debian ] && [ -z "$VERSION_ID" ] && echo >&2 "ERROR: Debian unstable not supported.
     You can try stable packages here:
     http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}" && exit 1
   $ sudo sh -c "echo 'deb http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}/Debian_${VERSION_ID}/ /' > /etc/apt/sources.list.d/kata-containers.list"
   $ curl -sL  http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}/Debian_${VERSION_ID}/Release.key | sudo apt-key add -
   $ sudo -E apt-get update
   $ sudo -E apt-get -y install kata-runtime kata-proxy kata-shim
   ```

2. Decide which container manager to use and select the corresponding link that follows:

   - [Docker](docker/debian-docker-install.md)
   - [Kubernetes](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#run-kata-containers-with-kubernetes)

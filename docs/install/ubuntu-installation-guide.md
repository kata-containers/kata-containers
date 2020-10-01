# Install Kata Containers on Ubuntu

1. Install the Kata Containers components with the following commands:

   ```bash
   $ ARCH=$(arch)
   $ BRANCH="${BRANCH:-master}"
   $ sudo sh -c "echo 'deb http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}/xUbuntu_$(lsb_release -rs)/ /' > /etc/apt/sources.list.d/kata-containers.list"
   $ curl -sL  http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}/xUbuntu_$(lsb_release -rs)/Release.key | sudo apt-key add -
   $ sudo -E apt-get update
   $ sudo -E apt-get -y install kata-runtime kata-proxy kata-shim
   ```

2. Decide which container manager to use and select the corresponding link that follows:
   - [Kubernetes](../Developer-Guide.md#run-kata-containers-with-kubernetes)

# Install Kata Containers on SLE

1. Install the Kata Containers components with the following commands:

   ```bash
   $ source /etc/os-release
   $ DISTRO_VERSION=$(sed "s/-/_/g" <<< "$VERSION")
   $ sudo -E zypper addrepo --refresh "https://download.opensuse.org/repositories/devel:/kubic/SLE_${DISTRO_VERSION}_Backports/devel:kubic.repo"
   $ sudo -E zypper -n --gpg-auto-import-keys install katacontainers
   ```

2. Decide which container manager to use and select the corresponding link that follows:
   - [Kubernetes](../Developer-Guide.md#run-kata-containers-with-kubernetes)

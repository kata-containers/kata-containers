# Install Kata Containers on openSUSE Leap 15.1

1. Install the Kata Containers components with the following commands:

   ```bash
   $ sudo -E zypper addrepo --refresh "https://download.opensuse.org/repositories/devel:/kubic/openSUSE_Leap_15.1/devel:kubic.repo"
   $ sudo -E zypper -n --gpg-auto-import-keys install katacontainers
   ```

2. Decide which container manager to use and select the corresponding link that follows:
   - [Kubernetes](../Developer-Guide.md#run-kata-containers-with-kubernetes)

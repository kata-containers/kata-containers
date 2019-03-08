# Install Kata Containers on SLES

> **Warning:**
>
> - The SLES packages are provided as a convenience to users until native
>   packages are available in SLES. However, they are **NOT** currently tested
>   (although openSUSE is) so caution should be exercised.
>
>   See https://github.com/kata-containers/ci/issues/126 for further details.

1. Install the Kata Containers components with the following commands:

   ```bash
   $ ARCH=$(arch)
   $ sudo -E zypper addrepo "http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/master/SLE_12_SP3/home:katacontainers:releases:${ARCH}:master.repo"
   $ sudo -E zypper -n --no-gpg-checks install kata-runtime kata-proxy kata-shim
   ```

2. Decide which container manager to use and select the corresponding link that follows:

   - [Docker](docker/sles-docker-install.md)
   - [Kubernetes](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#run-kata-containers-with-kubernetes)

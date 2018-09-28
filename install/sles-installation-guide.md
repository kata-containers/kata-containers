# Install Kata Containers on SLES

> **Notes:**
>
> - Kata Containers packages are available for [openSUSE\*](https://www.opensuse.org/)
>   version 43.2.
>
> - If you are installing on a system that already has Clear Containers or `runv` installed,
>   first read [the upgrading document](../Upgrading.md).
>
> - If you do not want to copy or type all these instructions by hand, you can use the
>   [`kata-manager`](https://github.com/kata-containers/tests/blob/master/cmd/kata-manager/kata-manager.sh)
>   script to install the packaged system including your chosen container
>   manager. Alternatively, you can generate a runnable shell script from
>   individual documents using the
>   [`kata-doc-to-script`](https://github.com/kata-containers/tests/blob/master/.ci/kata-doc-to-script.sh) script.

1. Install the Kata Containers components with the following commands:

   > **Note:** This installation channel is not secure since the repository currently
   > redirects download URLs to `http`.

   ```bash
   $ ARCH=$(arch)
   $ sudo -E zypper addrepo "http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/master/SLE_12_SP3/home:katacontainers:releases:${ARCH}:master.repo"
   $ sudo -E zypper -n --no-gpg-checks install kata-runtime kata-proxy kata-shim
   ```

2. Decide which container manager to use and select the corresponding link that follows:

   - [Docker](docker/sles-docker-install.md)
   - [Kubernetes](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#run-kata-containers-with-kubernetes)

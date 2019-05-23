# Install Kata Containers on openSUSE Leap/Tumbleweed

1. Install the Kata Containers components with the following commands:

   ```bash
   $ source /etc/os-release
   $ DISTRO_REPO=$(sed "s/ /_/g" <<< "$NAME")
   $ [ -n "$VERSION" ] && DISTRO_REPO+="_${VERSION}"
   $ ARCH=$(arch)
   $ BRANCH="${BRANCH:-master}"
   $ REPO_ALIAS="kata-${BRANCH}"
   $ PUBKEY="/tmp/rpm-signkey.pub"
   $ curl -SsL -o "$PUBKEY" "https://raw.githubusercontent.com/kata-containers/tests/master/data/rpm-signkey.pub"
   $ sudo -E rpm --import "$PUBKEY"
   $ zypper lr "$REPO_ALIAS" && sudo -E zypper -n removerepo "$REPO_ALIAS"
   $ sudo -E zypper addrepo --refresh "http://download.opensuse.org/repositories/home:/katacontainers:/releases:/${ARCH}:/${BRANCH}/${DISTRO_REPO}/" "$REPO_ALIAS"
   $ sudo -E zypper -n install kata-runtime
   ```

2. Decide which container manager to use and select the corresponding link that follows:

   - [Docker](docker/opensuse-docker-install.md)
   - [Kubernetes](https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#run-kata-containers-with-kubernetes)

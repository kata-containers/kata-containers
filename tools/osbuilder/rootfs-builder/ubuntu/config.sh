# This is a configuration file add extra variables to
#
# Copyright (c) 2018  Yash Jain
#
# SPDX-License-Identifier: Apache-2.0
# be used by build_rootfs() from rootfs_lib.sh the variables will be
# loaded just before call the function. For more information see the
# rootfs-builder/README.md file.

OS_VERSION=${OS_VERSION:-20.04}
# This should be Ubuntu's code name, e.g. "focal" (Focal Fossa) for 20.04
OS_NAME=${OS_NAME:-"focal"}

# packages to be installed by default
# Note: ca-certificates is required for confidential containers
# to pull the container image on the guest
PACKAGES="systemd iptables init kmod ca-certificates"
EXTRA_PKGS+=" chrony"

DEBOOTSTRAP=${PACKAGE_MANAGER:-"debootstrap"}

case $(uname -m) in
	x86_64) ARCHITECTURE="amd64";;
	ppc64le) ARCHITECTURE="ppc64el";;
	aarch64) ARCHITECTURE="arm64";;
	s390x)	ARCHITECTURE="s390x";;
	(*) die "$(uname -m) not supported "
esac

# Init process must be one of {systemd,kata-agent}
INIT_PROCESS=systemd
# List of zero or more architectures to exclude from build,
# as reported by  `uname -m`
ARCH_EXCLUDE_LIST=()

[ "$SECCOMP" = "yes" ] && PACKAGES+=" libseccomp2" || true
[ "$SKOPEO" = "yes" ] && PACKAGES+=" libgpgme11" || true

if [ "${AA_KBC}" == "eaa_kbc" ] && [ "${ARCH}" == "x86_64" ]; then
    AA_KBC_EXTRAS="
RUN echo 'deb [arch=amd64] http://mirrors.openanolis.cn/inclavare-containers/ubuntu20.04 bionic main' \| tee /etc/apt/sources.list.d/inclavare-containers.list; \
    wget -qO - http://mirrors.openanolis.cn/inclavare-containers/ubuntu20.04/DEB-GPG-KEY.key  \| apt-key add -; \
    apt-get update; \
    apt-get install -y rats-tls
"
fi

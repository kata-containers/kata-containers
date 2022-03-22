# Copyright (c) 2018 Yash Jain, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME=ubuntu
# This should be Ubuntu's code name, e.g. "focal" (Focal Fossa) for 20.04
OS_VERSION=${OS_VERSION:-focal}
PACKAGES=chrony
[ "$AGENT_INIT" = no ] && PACKAGES+=" init"
[ "$SECCOMP" = yes ] && PACKAGES+=" libseccomp2"
# packages to be installed by default
# Note: ca-certificates is required for confidential containers
# to pull the container image on the guest
PACKAGES+=" ca-certificates"
REPO_URL=http://ports.ubuntu.com

case "$ARCH" in
	aarch64) DEB_ARCH=arm64;;
	ppc64le) DEB_ARCH=ppc64el;;
	s390x) DEB_ARCH="$ARCH";;
	x86_64) DEB_ARCH=amd64; REPO_URL=http://archive.ubuntu.com/ubuntu;;
	*) die "$ARCH not supported"
esac


if [ "$(uname -m)" != "$ARCH" ]; then
	case "$ARCH" in
		ppc64le) cc_arch=powerpc64le;;
		x86_64) cc_arch=x86-64;;
		*) cc_arch="$ARCH"
	esac
	export CC="$cc_arch-linux-gnu-gcc"
fi

[ "$SKOPEO" = "yes" ] && PACKAGES+=" libgpgme11" || true

if [ "${AA_KBC}" == "eaa_kbc" ] && [ "${ARCH}" == "x86_64" ]; then
    AA_KBC_EXTRAS="
RUN echo 'deb [arch=amd64] http://mirrors.openanolis.cn/inclavare-containers/ubuntu20.04 bionic main' \| tee /etc/apt/sources.list.d/inclavare-containers.list; \
    wget -qO - http://mirrors.openanolis.cn/inclavare-containers/ubuntu20.04/DEB-GPG-KEY.key  \| apt-key add -; \
    apt-get update; \
    apt-get install -y rats-tls
"
fi
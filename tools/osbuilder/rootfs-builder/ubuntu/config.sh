# Copyright (c) 2018 Yash Jain, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME=ubuntu
# This should be Ubuntu's code name, e.g. "focal" (Focal Fossa) for 20.04
OS_VERSION=${OS_VERSION:-focal}
PACKAGES="chrony iptables dbus kmod"
[ "$AGENT_INIT" = no ] && PACKAGES+=" init"
[ "$MEASURED_ROOTFS" = yes ] && PACKAGES+=" cryptsetup-bin e2fsprogs"
[ "$SECCOMP" = yes ] && PACKAGES+=" libseccomp2"
[ "$DM_VERITY" = yes ] && PACKAGES+=" udev dmsetup"
REPO_URL=http://ports.ubuntu.com

case "$ARCH" in
	aarch64) DEB_ARCH=arm64;;
	ppc64le) DEB_ARCH=ppc64el;;
	s390x) DEB_ARCH="$ARCH";;
	x86_64) DEB_ARCH=amd64; REPO_URL=http://archive.ubuntu.com/ubuntu;;
	*) die "$ARCH not supported"
esac

if [ "${AA_KBC}" == "eaa_kbc" ] && [ "${ARCH}" == "x86_64" ]; then
	source /etc/os-release

	if [ "${VERSION_ID}" == "20.04" ]; then
		PACKAGES+=" apt gnupg"
		AA_KBC_EXTRAS="
RUN echo 'deb [arch=amd64] http://mirrors.openanolis.cn/inclavare-containers/ubuntu${VERSION_ID} ${OS_VERSION} main' \| tee /etc/apt/sources.list.d/inclavare-containers.list; \
    curl -L http://mirrors.openanolis.cn/inclavare-containers/ubuntu${VERSION_ID}/DEB-GPG-KEY.key \| apt-key add -; \
    echo 'deb [arch=amd64] https://download.01.org/intel-sgx/sgx_repo/ubuntu ${OS_VERSION} main' \| tee /etc/apt/sources.list.d/intel-sgx.list; \
    curl -L https://download.01.org/intel-sgx/sgx_repo/ubuntu/intel-sgx-deb.key \| apt-key add -; \
    apt-get update; \
    apt-get install -y rats-tls-tdx libtdx-attest=1.15\*
"
	else
		echo "rats-tls-tdx is only provided for Ubuntu 20.04, there's yet no packages for Ubuntu ${VERSION_ID}"
	fi
fi

if [ "${AA_KBC}" == "cc_kbc_tdx" ] && [ "${ARCH}" == "x86_64" ]; then
	source /etc/os-release

	if [ "${OS_VERSION}" == "focal" ] || [ "${OS_VERSION}" == "20.04" ]; then
		PACKAGES+=" apt gnupg"
		AA_KBC_EXTRAS="
	RUN echo 'deb [arch=amd64] https://download.01.org/intel-sgx/sgx_repo/ubuntu ${OS_VERSION} main' \| tee /etc/apt/sources.list.d/intel-sgx.list; \
		curl -L https://download.01.org/intel-sgx/sgx_repo/ubuntu/intel-sgx-deb.key \| apt-key add -; \
		apt-get update; \
		apt-get install -y libtdx-attest=1.15\* libtdx-attest-dev=1.15\* clang
	"
	else
		echo "libtdx-attest-dev is only provided for Ubuntu 20.04; not for ${OS_VERSION}"
		exit 1
	fi
fi

if [ "$(uname -m)" != "$ARCH" ]; then
	case "$ARCH" in
		ppc64le) cc_arch=powerpc64le;;
		x86_64) cc_arch=x86-64;;
		*) cc_arch="$ARCH"
	esac
	export CC="$cc_arch-linux-gnu-gcc"
fi

#!/usr/bin/env bash
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0

# Variables are consumed externally by the osbuilder rootfs build system.
# shellcheck disable=SC2034

OS_NAME=ubuntu
# Ubuntu code name (e.g. "resolute"), passed down from the guest image release so
# the devkit matches the base userspace ABI (glibc).
OS_VERSION=${OS_VERSION:-""}
[[ -z "${OS_VERSION}" ]] && echo "OS_VERSION is required, but was not set" && exit 1

# Debug tools prebaked so the extension works offline; `apt install <pkg>` pulls
# anything else on demand inside the overlay. Kept lean: heavier tools are
# pulled on demand.
PACKAGES=(
	# apt is not in the "required" priority set mmdebstrap installs, so it has
	# to be asked for explicitly (it also pulls gpgv for repo verification).
	apt
	ca-certificates
	# Bootstraps the overlay/chroot from the shell-less guest base, see
	# devkit-init.sh.
	busybox-static
	bash
	# Inspect the guest's devices and namespaces from within the chroot.
	pciutils
	util-linux
	strace
	ltrace
	lsof
	psmisc
	procps
	tcpdump
	iproute2
	dnsutils
	netcat-openbsd
	curl
	wget
	file
	less
)

# ltrace and busybox-static live in universe.
REPO_COMPONENTS=${REPO_COMPONENTS:-"main universe"}

# shellcheck disable=SC2154
case "${ARCH}" in
	aarch64) DEB_ARCH=arm64;;
	ppc64le) DEB_ARCH=ppc64el;;
	s390x) DEB_ARCH="${ARCH}";;
	x86_64) DEB_ARCH=amd64; REPO_URL=${REPO_URL_X86_64:-${REPO_URL:-http://archive.ubuntu.com/ubuntu}};;
	*) die "${ARCH} not supported"
esac
REPO_URL=${REPO_URL:-http://ports.ubuntu.com}

#!/usr/bin/env bash
#
# Copyright (c) 2018 Yash Jain, 2022 IBM Corp.
#
# SPDX-License-Identifier: Apache-2.0

# Variables are used externally by the rootfs build system
# shellcheck disable=SC2034

OS_NAME=ubuntu
# This should be Ubuntu's code name, e.g. "focal" (Focal Fossa) for 20.04
OS_VERSION=${OS_VERSION:-""}
[[ -z "${OS_VERSION}" ]] && echo "OS_VERSION is required, but was not set" && exit 1
PACKAGES="chrony iptables dbus"
# shellcheck disable=SC2154
[[ "${AGENT_INIT}" = no ]] && PACKAGES+=" init"
# cryptsetup-bin and e2fsprogs are installed unconditionally:
#  - cryptsetup-bin is required by CDH's secure storage feature (encrypted
#    volumes) in confidential guests.
#  - e2fsprogs (mke2fs/mkfs.ext4) is required both by CDH secure storage and by
#    the plain ephemeral storage feature, which is not confidential-only.
PACKAGES+=" cryptsetup-bin e2fsprogs"
# shellcheck disable=SC2154
[[ "${SECCOMP}" = yes ]] && PACKAGES+=" libseccomp2"
[[ "$(uname -m)" = "s390x" ]] && PACKAGES+=" libcurl4 libnghttp2-14"
REPO_COMPONENTS=${REPO_COMPONENTS:-main}

# shellcheck disable=SC2154
case "${ARCH}" in
	aarch64) DEB_ARCH=arm64;;
	ppc64le) DEB_ARCH=ppc64el;;
	s390x) DEB_ARCH="${ARCH}";;
	x86_64) DEB_ARCH=amd64; REPO_URL=${REPO_URL_X86_64:-${REPO_URL:-http://archive.ubuntu.com/ubuntu}};;
	*) die "${ARCH} not supported"
esac
REPO_URL=${REPO_URL:-http://ports.ubuntu.com}

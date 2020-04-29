#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME="Clear"
REPO_NAME="clear"

OS_VERSION=${OS_VERSION:-latest}

clr_url="https://download.clearlinux.org"

# resolve version
[ "${OS_VERSION}" = "latest" ] && OS_VERSION=$(curl -sL "${clr_url}/latest")

BASE_URL="${clr_url}/releases/${OS_VERSION}/${REPO_NAME}/${ARCH}/os/"

PACKAGES="libudev0-shim kmod-bin"

#Optional packages:
# systemd: An init system that will start kata-agent if kata-agent
#          itself is not configured as init process.
[ "$AGENT_INIT" == "no" ] && PACKAGES+=" systemd chrony iptables-bin util-linux-bin" || true

# Init process must be one of {systemd,kata-agent}
INIT_PROCESS=systemd
# List of zero or more architectures to exclude from build,
# as reported by  `uname -m`
ARCH_EXCLUDE_LIST=( aarch64 ppc64le s390x )

[ "$SECCOMP" = "yes" ] && PACKAGES+=" libseccomp" || true

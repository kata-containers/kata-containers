# This is a configuration file add extra variables to
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
# be used by build_rootfs() from rootfs_lib.sh the variables will be
# loaded just before call the function. For more information see the
# rootfs-builder/README.md file.

OS_VERSION=${OS_VERSION:-latest}
OS_NAME=${OS_NAME:-"gentoo"}

# packages to be installed by default
PACKAGES="sys-apps/systemd net-firewall/iptables net-misc/chrony"

# Init process must be one of {systemd,kata-agent}
INIT_PROCESS=systemd
# List of zero or more architectures to exclude from build,
# as reported by  `uname -m`
ARCH_EXCLUDE_LIST=( aarch64 ppc64le s390x )

[ "$SECCOMP" = "yes" ] && PACKAGES+=" sys-libs/libseccomp" || true

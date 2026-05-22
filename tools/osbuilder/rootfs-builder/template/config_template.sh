#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# This is a configuration file add extra variables to
# be used by build_rootfs() from rootfs_lib.sh the variables will be
# loaded just before call the function. For more information see the
# rootfs-builder/README.md file.

OS_VERSION=${OS_VERSION:-DEFAULT_VERSION}

# shellcheck disable=SC2034
PACKAGES="systemd iptables udevlib.so"

# Init process must be one of {systemd,kata-agent}
# shellcheck disable=SC2034
INIT_PROCESS=systemd
# List of zero or more architectures to exclude from build,
# as reported by  `uname -m`
# shellcheck disable=SC2034
ARCH_EXCLUDE_LIST=()
# [When uncommented,] Allow the build to fail without generating an error
# For more info see: https://github.com/kata-containers/osbuilder/issues/190
#BUILD_CAN_FAIL=1

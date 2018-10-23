#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME="Alpine"

OS_VERSION=${OS_VERSION:-v3.7}

BASE_PACKAGES="alpine-base"

# Alpine mirror to use
# See a list of mirrors at http://nl.alpinelinux.org/alpine/MIRRORS.txt
MIRROR=http://dl-5.alpinelinux.org/alpine

# Mandatory Packages that must be installed
#  - iptables: Need by Kata agent
PACKAGES="iptables"

# Init process must be one of {systemd,kata-agent}
INIT_PROCESS=kata-agent
# List of zero or more architectures to exclude from build,
# as reported by  `uname -m`
ARCH_EXCLUDE_LIST=()

#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

OS_NAME="Alpine"

OS_VERSION=${OS_VERSION:-3.15}

BASE_PACKAGES="alpine-base"

# Alpine mirror to use
# See a list of mirrors at http://nl.alpinelinux.org/alpine/MIRRORS.txt
MIRROR=https://dl-5.alpinelinux.org/alpine

PACKAGES="iptables ip6tables"

# Init process must be one of {systemd,kata-agent}
INIT_PROCESS=kata-agent
# List of zero or more architectures to exclude from build,
# as reported by  `uname -m`
ARCH_EXCLUDE_LIST=()

[ "$SECCOMP" = "yes" ] && PACKAGES+=" libseccomp" || true

#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# shellcheck disable=SC2034
OS_NAME="Alpine"

OS_VERSION=${OS_VERSION:-3.18}

# shellcheck disable=SC2034
BASE_PACKAGES="alpine-base"

# Alpine mirror to use
# See a list of mirrors at http://nl.alpinelinux.org/alpine/MIRRORS.txt
# shellcheck disable=SC2034
MIRROR=http://dl-cdn.alpinelinux.org/alpine/

PACKAGES="bash iptables ip6tables kmod"

# Init process must be one of {systemd,kata-agent}
# shellcheck disable=SC2034
INIT_PROCESS=kata-agent
# List of zero or more architectures to exclude from build,
# as reported by  `uname -m`
# shellcheck disable=SC2034
ARCH_EXCLUDE_LIST=()

# shellcheck disable=SC2154
if [[ "${SECCOMP}" = "yes" ]]; then PACKAGES+=" libseccomp"; fi

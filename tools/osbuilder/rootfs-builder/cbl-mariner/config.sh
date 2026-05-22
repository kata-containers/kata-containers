#!/usr/bin/env bash
#
# Copyright (c) 2023 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

# shellcheck disable=SC2034
OS_NAME=cbl-mariner
OS_VERSION=${OS_VERSION:-3.0}
# shellcheck disable=SC2034
LIBC="gnu"
PACKAGES="kata-packages-uvm"
# shellcheck disable=SC2154
if [[ "${AGENT_INIT}" = "no" ]]; then PACKAGES+=" systemd"; fi
# shellcheck disable=SC2154
if [[ "${SECCOMP}" = "yes" ]]; then PACKAGES+=" libseccomp"; fi

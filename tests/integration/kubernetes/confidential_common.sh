#!/usr/bin/env bats
# Copyright 2022-2023 Advanced Micro Devices, Inc.
# Copyright 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# This function relies on `KATA_HYPERVISOR` being an environment variable
# and returns the remote command to be executed to that specific hypervisor
# in order to identify whether the workload is running on a TEE environment
function get_remote_command_per_hypervisor() {
    declare -A REMOTE_COMMAND_PER_HYPERVISOR
    REMOTE_COMMAND_PER_HYPERVISOR[qemu-tdx]="/usr/bin/cpuid | grep TDX_GUEST"

    echo "${REMOTE_COMMAND_PER_HYPERVISOR[${KATA_HYPERVISOR}]}"
}

#!/usr/bin/env bats

# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/../../../common.bash"
load "${BATS_TEST_DIRNAME}/../setup_common.sh"

setup_file() {
    info "setup"
}

@test "Test GetGuestDetails: Boot qemu pod vm and run GetGuestDetails" {
    info "Boot qemu vm, establish connection with agent inside the vm and send GetGuestDetails command"
    local test_dir=$(mktemp -d)
    pushd $test_dir
    local cmds=()
    cmds+=("--vm qemu -c GetGuestDetails")
    run_agent_ctl "${cmds[@]}"
    popd
    rm -rf $test_dir
}

@test "Test GetGuestDetails: Boot cloud hypervisor pod vm and run GetGuestDetails" {
    info "Boot cloud hypervisor vm, establish connection with agent inside the vm and send GetGuestDetails command"
    local cmds=()
    cmds+=("--vm cloud-hypervisor -c GetGuestDetails")
    run_agent_ctl "${cmds[@]}"
}

teardown_file() {
    info "teardown"
    sudo rm -r /run/kata/agent-ctl-testvm || echo "Failed to clean /run/kata/agent-ctl-testvm"
    sudo rm -r /run/kata-containers/ || echo "Failed to clean /run/kata-containers"
}

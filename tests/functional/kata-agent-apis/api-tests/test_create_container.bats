#!/usr/bin/env bats

# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/../../../common.bash"
load "${BATS_TEST_DIRNAME}/../setup_common.sh"

setup_file() {
    info "setup"
}

@test "Test CreateContainer API: Create a container" {
    info "Create a container"
    sandbox_id=$RANDOM
    container_id="test_container_${RANDOM}"
    
    local cmds=()
    cmds+="-c 'CreateSandbox json://{\"sandbox_id\": \"$sandbox_id\"}'"
    run_agent_ctl "${cmds[@]}"

    local image="ghcr.io/linuxcontainers/alpine:latest"
    local cmds=()
    cmds+="-c 'CreateContainer json://{\"image\": \"$image\", \"id\": \"$container_id\"}'"
    run_agent_ctl "${cmds[@]}"
    info "Container created successfully."

    local cmds=()
    cmds+="-c 'StartContainer json://{\"container_id\": \"$container_id\"}'"
    run_agent_ctl "${cmds[@]}"
    info "Container process started"

    local cmds=()
    cmds+="-c 'RemoveContainer json://{\"container_id\": \"$container_id\"}'"
    run_agent_ctl "${cmds[@]}"
    info "Container removed."
}

teardown_file() {
    info "teardown"
    sudo rm -r /run/kata-containers/ || echo "Failed to clean /run/kata-containers"
}

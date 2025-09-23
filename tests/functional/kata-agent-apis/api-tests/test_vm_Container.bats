#!/usr/bin/env bats

# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/../../../common.bash"
load "${BATS_TEST_DIRNAME}/../setup_common.sh"

setup_file() {
    info "setup"
}

@test "Test Container Lifecycle: Boot qemu pod vm and run a container" {
    info "Boot qemu vm, establish connection with agent inside the vm and send container commands"
    local test_dir=$(mktemp -d)
    pushd $test_dir
    sandbox_id=$RANDOM
    container_id="test_container_${RANDOM}"

    local image="ghcr.io/linuxcontainers/alpine:latest"

    local cmds=()
    cmds+=("--vm qemu")
    cmds+=" -c 'CreateSandbox json://{\"sandbox_id\": \"$sandbox_id\"}'"
    cmds+=" 'CreateContainer json://{\"image\": \"$image\", \"id\": \"$container_id\"}'"
    cmds+=" 'StartContainer json://{\"container_id\": \"$container_id\"}'"
    cmds+=" 'RemoveContainer json://{\"container_id\": \"$container_id\"}'"

    run_agent_ctl "${cmds[@]}"
    popd
    rm -rf $test_dir
}

@test "Test Container Lifecycle: Boot cloud hypervisor pod vm and run a container" {
    info "Boot cloud hypervisor vm, establish connection with agent inside the vm and send container commands"
    sandbox_id=$RANDOM
    container_id="test_container_${RANDOM}"

    local image="ghcr.io/linuxcontainers/alpine:latest"

    local cmds=()
    cmds+=("--vm cloud-hypervisor")
    cmds+=" -c 'CreateSandbox json://{\"sandbox_id\": \"$sandbox_id\"}'"
    cmds+=" 'CreateContainer json://{\"image\": \"$image\", \"id\": \"$container_id\"}'"
    cmds+=" 'StartContainer json://{\"container_id\": \"$container_id\"}'"
    cmds+=" 'RemoveContainer json://{\"container_id\": \"$container_id\"}'"

    run_agent_ctl "${cmds[@]}"
}

teardown_file() {
    info "teardown"
    sudo rm -r /run/kata/agent-ctl-testvm || echo "Failed to clean /run/kata/agent-ctl-testvm"
    sudo rm -r /run/kata-containers/ || echo "Failed to clean /run/kata-containers"
}

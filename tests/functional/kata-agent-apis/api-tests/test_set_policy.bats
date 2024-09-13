#!/usr/bin/env bats

# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/../../../common.bash"
load "${BATS_TEST_DIRNAME}/../setup_common.sh"

setup_file() {
    info "setup"
}

@test "Test SetPolicy API: Set allow all policy" {
    info "Upload policy document from under src/kata-opa"
    repo_root_dir="${BATS_TEST_DIRNAME}/../../../../"
    policy_dir="${repo_root_dir}/src/kata-opa"
    policy_file="${policy_dir}/allow-all.rego"
    local cmds=()
    cmds+=("-c 'SetPolicy json://{\"policy_file\": \"$policy_file\"}'")
    run_agent_ctl "${cmds[@]}"
}

@test "Test SetPolicy API: Block CopyFile in policy" {
    policy_file=$(mktemp)
    deny_single_api_in_policy ${policy_file} "CopyFileRequest"
    local cmds=()
    cmds+=("-c 'SetPolicy json://{\"policy_file\": \"$policy_file\"}'")
    run_agent_ctl "${cmds[@]}"

    src_file=$(mktemp)
    local cmds=()
    cmds+=("-c 'CopyFile json://{\"src\": \"$src_file\", \"dest\":\"/run/kata-containers/foo\"}'")
    run run_agent_ctl "${cmds[@]}"
    [ "$status" -ne 0 ]

    rm $src_file
    rm $policy_file
}

teardown_file() {
    info "teardown"
    sudo rm -r /run/kata-containers/ || echo "Failed to clean /run/kata-containers"
}

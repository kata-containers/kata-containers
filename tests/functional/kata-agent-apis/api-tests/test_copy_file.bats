#!/usr/bin/env bats

# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/../../../common.bash"
load "${BATS_TEST_DIRNAME}/../setup_common.sh"

setup_file() {
    info "setup"
}

@test "Test CopyFile API: Copy a file to /run/kata-containers/shared/containers" {
    info "Copy file to /run/kata-containers/shared/containers"
    src_file=$(mktemp)
    local cmds=()
    cmds+=("-c 'CopyFile json://{\"src\": \"$src_file\", \"dest\":\"/run/kata-containers/shared/containers/foo\"}'")
    run_agent_ctl "${cmds[@]}"
    rm $src_file
}

@test "Test CopyFile API: Copy a symlink to /run/kata-containers/shared/containers" {
    info "Copy symlink to /run/kata-containers/shared/containers"
    src_file=$(mktemp)
    link_file="/tmp/link"
    ln -s $src_file $link_file
    local cmds=()
    cmds+=("-c 'CopyFile json://{\"src\": \"$link_file\", \"dest\":\"/run/kata-containers/shared/containers/link\"}'")
    run_agent_ctl "${cmds[@]}"
    unlink $link_file
    rm $src_file
}

@test "Test CopyFile API: Copy a directory to /run/kata-containers/shared/containers" {
    info "Copy directory to /run/kata-containers/shared/containers"
    src_dir=$(mktemp -d)
    local cmds=()
    cmds+=("-c 'CopyFile json://{\"src\": \"$src_dir\", \"dest\":\"/run/kata-containers/shared/containers/dir\"}'")
    run_agent_ctl "${cmds[@]}"
    rmdir $src_dir
}

@test "Test CopyFile API: Copy a file to an unallowed destination" {
    info "Copy file to /tmp"
    src_file=$(mktemp)
    local cmds=()
    cmds+=("-c 'CopyFile json://{\"src\": \"$src_file\", \"dest\":\"/tmp/foo\"}'")
    run run_agent_ctl "${cmds[@]}"
    [ "$status" -ne 0 ]
    rm $src_file
}

@test "Test CopyFile API: Copy a file to an unallowed destination beneath /run/kata-containers" {
    # This is a regression test, copying files to /run/kata-containers used to be allowed, but the
    # implementation is more strict now and requires paths to be under
    # /run/kata-containers/shared/containers.
    info "Copy file to /tmp"
    src_file=$(mktemp)
    local cmds=()
    cmds+=("-c 'CopyFile json://{\"src\": \"$src_file\", \"dest\":\"/run/kata-containers/shared/foo\"}'")
    run run_agent_ctl "${cmds[@]}"
    [ "$status" -ne 0 ]
    rm $src_file
}

@test "Test CopyFile API: Copy a large file to /run/kata-containers/shared/containers" {
    info "Copy large file to /run/kata-containers/shared/containers"
    src_file="/tmp/large_file_2M.txt"
    truncate -s 2M $src_file
    local cmds=()
    cmds+=("-c 'CopyFile json://{\"src\": \"$src_file\", \"dest\":\"/run/kata-containers/shared/containers/large_file_2M.txt\"}'")
    run_agent_ctl "${cmds[@]}"
    rm $src_file
}

teardown_file() {
    info "teardown"
    sudo rm -r /run/kata-containers/shared/containers/ || echo "Failed to clean /run/kata-containers/shared/containers"
}

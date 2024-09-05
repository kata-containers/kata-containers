#!/usr/bin/env bats

# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

load "${BATS_TEST_DIRNAME}/../../../common.bash"
load "${BATS_TEST_DIRNAME}/../setup_common.sh"

setup_file() {
    info "setup"
}

@test "Dummy test" {
    info "placeholder"
}

teardown_file() {
    info "teardown"
}

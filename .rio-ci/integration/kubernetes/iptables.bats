#!/usr/bin/env bats
#
# Copyright (c) 2021 Apple Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
        export HOST_PASSWORD="Rekur\$i0n"
        TEST_SCRIPT="iptables-test.sh"
        # scp test to the node
        sshpass -p "${HOST_PASSWORD}" scp $TEST_SCRIPT "${CLUSTER_NAME}":"/tmp/${TEST_SCRIPT}"
}

# Skip on aarch64 due to missing cpu hotplug related functionality.
@test "Check iptables API on a container" {
        # run test:
        sshpass -p "${HOST_PASSWORD}" ssh "${CLUSTER_NAME}" "sudo bash -x /tmp/${TEST_SCRIPT}"
}

teardown() {
        sshpass -p "${HOST_PASSWORD}" ssh  "${CLUSTER_NAME}" "sudo rm /tmp/${TEST_SCRIPT}"
}

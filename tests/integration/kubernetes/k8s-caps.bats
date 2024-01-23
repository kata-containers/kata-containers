#!/usr/bin/env bats
#
# Copyright (c) 2021 Apple Inc.
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
        pod_name="pod-caps"
        get_pod_config_dir
        yaml_file="${pod_config_dir}/${pod_name}.yaml"

# We expect the capabilities mask to very per distribution, runtime
# configuration. Even for this, we should expect a few common items to
# not be set in the mask unless we are failing to apply capabilities. If
# we fail to configure, we'll see all bits set for permitted: 0x03fffffffff
# We do expect certain parts of the mask to be common when we set appropriately:
#  b20..b23 should be cleared for all (no CAP_SYS_{PACCT, ADMIN, NICE, BOOT})
#  b0..b11 are consistent across the distros:
#  0x5fb: 0101 1111 1011
#         | |        \- should be cleared (CAP_DAC_READ_SEARCH)
#         |  \- should be cleared (CAP_LINUX_IMMUTABLE)
#          \- should be cleared (CAP_NET_BROADCAST)
# Example match:
#   CapPrm:       00000000a80425fb
        expected="CapPrm.*..0..5fb$"
}

@test "Check capabilities of pod" {
        # TODO: disabled due to #8850
        # auto_generate_policy "${yaml_file}"

        # Create pod
        kubectl create -f "${yaml_file}"
        # Check pod creation
        kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

        # Verify expected capabilities for the running container. Add retry to ensure
        # that the container had time to execute:
        wait_time=5
        sleep_time=1
        cmd="kubectl logs $pod_name | grep -q $expected"
        waitForProcess "$wait_time" "$sleep_time" "$cmd"

        # Verify expected capabilities from exec context:
        kubectl exec "$pod_name" -- sh -c "cat /proc/self/status" | grep -q "$expected"
}

teardown() {
        # Debugging information
        echo "expected capability mask:"
        echo "$expected"
        echo "observed: "
        kubectl logs "pod/$pod_name"
        kubectl exec "$pod_name" -- sh -c "cat /proc/self/status | grep Cap"
        kubectl delete pod "$pod_name"
}

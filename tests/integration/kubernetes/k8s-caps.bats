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

        yaml_file="${pod_config_dir}/pod-caps.yaml"
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	command="cat /proc/self/status"
	exec_command=(sh -c "${command}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

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
        kubectl exec "$pod_name" -- "${exec_command[@]}" | grep -q "$expected"
}

teardown() {
        # Debugging information
        echo "expected capability mask:"
        echo "$expected"
        echo "observed: "
        kubectl logs "pod/$pod_name"
        kubectl exec "$pod_name" -- "${exec_command[@]}" | grep Cap
        kubectl delete pod "$pod_name"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}

#!/usr/bin/env bats
#
# Copyright (c) 2022 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
TEST_INITRD="${TEST_INITRD:-no}"

setup() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	pod_name="test-file-volume"
	container_name="busybox-file-volume-container"
	node="$(get_one_kata_node)"
	tmp_file=$(mktemp -u /tmp/file-volume-test-foo.XXXXX)
	exec_host "$node" touch $tmp_file
	mount_path="/tmp/foo.txt"
	file_body="test"
	get_pod_config_dir

	# Write test body to temp file
	exec_host "$node" "echo "$file_body" > $tmp_file"

	# Create test yaml
	test_yaml="${pod_config_dir}/test-pod-file-volume.yaml"

	sed -e "s|HOST_FILE|$tmp_file|" ${pod_config_dir}/pod-file-volume.yaml > "${test_yaml}"
	sed -i "s|MOUNT_PATH|$mount_path|" "${test_yaml}"
	sed -i "s|NODE|$node|" "${test_yaml}"

	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	command=(cat "$mount_path")
	add_exec_to_policy_settings "${policy_settings_dir}" "${command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${test_yaml}"

	return 0
}

@test "Test readonly volume for pods" {
	# Create pod
	kubectl create -f "${test_yaml}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Validate file volume body inside the pod
	file_in_container=$(kubectl exec $pod_name -- "${command[@]}")
	[ "$file_body" == "$file_in_container" ]
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "firecracker" ] && skip "test not working see: ${fc_limitations}"
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: ${fc_limitations}"

	kubectl describe pod "$pod_name"

	kubectl delete pod "$pod_name"
	exec_host "$node" rm -f $tmp_file
	rm -f "${test_yaml}"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}

#!/usr/bin/env bats
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"
load "${BATS_TEST_DIRNAME}/confidential_common.sh"
TEST_INITRD="${TEST_INITRD:-no}"

setup() {
	if ! is_shared_fs_none_runtime_class; then
		skip "Test requires a shared_fs=none runtime class"
	fi

	setup_common || die "setup_common failed"
	pod_name="test-large-file-volume"
	container_name="busybox-large-file-volume-container"
	node="$(get_one_kata_node)"
	tmp_file=$(mktemp -u /tmp/large-file-volume-test-foo.XXXXX)
	exec_host "$node" touch $tmp_file
	mount_path="/tmp/foo.txt"

    # Create a 2 GB temporary file
    tmp_file_size=2147483648
	exec_host "$node" "dd if=/dev/zero of=$tmp_file bs=1M count=2048"

	# Create test yaml
	test_yaml="${pod_config_dir}/test-pod-large-file-volume.yaml"

    sed \
		-e "s|HOST_FILE|$tmp_file|" \
		-e "s|MOUNT_PATH|$mount_path|" \
		-e "s|NODE|$node|" \
		"${pod_config_dir}/pod-large-file-volume.yaml" > "${test_yaml}"

	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	command=(stat -c "%s" "$mount_path")
	add_exec_to_policy_settings "${policy_settings_dir}" "${command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${test_yaml}"

	return 0
}

@test "Test large file volume for pods" {
	# Create pod
	kubectl create -f "${test_yaml}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Validate file volume size inside the pod
	file_size_in_container=$(kubectl exec $pod_name -- "${command[@]}")
	[ "$file_size_in_container" == "$tmp_file_size" ]
}

teardown() {
	if ! is_shared_fs_none_runtime_class; then
		return
	fi

	kubectl describe pod "$pod_name"

	kubectl delete pod "$pod_name"
	exec_host "$node" rm -f $tmp_file
	rm -f "${test_yaml}"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
	teardown_common "${node}" "${node_start_time:-}"
}

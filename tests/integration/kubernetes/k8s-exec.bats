#!/usr/bin/env bats
#
# Copyright (c) 2020 Ant Financial
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir
	pod_name="busybox"
	first_container_name="first-test-container"
	second_container_name="second-test-container"

	test_yaml_file="${pod_config_dir}/test-busybox-pod.yaml"
	cp "$pod_config_dir/busybox-pod.yaml" "${test_yaml_file}"

	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	date_command="date"
	add_exec_to_policy_settings "${policy_settings_dir}" "${date_command}"
	sh_command="sh"
	add_exec_to_policy_settings "${policy_settings_dir}" "${sh_command}"
	env_command="env"
	add_exec_to_policy_settings "${policy_settings_dir}" "${env_command}"

	allowed_requests=(
		"CloseStdinRequest"
		"ReadStreamRequest"
		"WriteStreamRequest"
	)
	add_requests_to_policy_settings "${policy_settings_dir}" "${allowed_requests[@]}"

	auto_generate_policy "${policy_settings_dir}" "${test_yaml_file}"
}

@test "Kubectl exec" {
	# Create the pod
	kubectl create -f "${test_yaml_file}"

	# Get pod specification
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Run commands in Pod
	## Cases for -it options
	# TODO: enable -i option after updated to new CRI-O
	# see: https://github.com/kata-containers/tests/issues/2770
	# kubectl exec -i "$pod_name" -- ls -tl /
	# kubectl exec -it "$pod_name" -- ls -tl /
	kubectl exec "$pod_name" -- "$date_command"

	## Case for stdin
	kubectl exec -i "$pod_name" -- "$sh_command" <<-EOF
echo abc > /tmp/abc.txt
grep abc /tmp/abc.txt
exit
EOF

	## Case for return value
	### Command return non-zero code
	run bash -c "kubectl exec -i $pod_name -- "$sh_command" <<-EOF
exit 123
EOF"
	echo "run status: $status" 1>&2
	echo "run output: $output" 1>&2
	[ "$status" -eq 123 ]

	## Cases for target container
	### First container
	container_name=$(kubectl exec $pod_name -c $first_container_name -- $env_command | grep CONTAINER_NAME)
	[ "$container_name" == "CONTAINER_NAME=$first_container_name" ]

	### Second container
	container_name=$(kubectl exec $pod_name -c $second_container_name -- $env_command | grep CONTAINER_NAME)
	[ "$container_name" == "CONTAINER_NAME=$second_container_name" ]

}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"

	rm "${test_yaml_file}"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}

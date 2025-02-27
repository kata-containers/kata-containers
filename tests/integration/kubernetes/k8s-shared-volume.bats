#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" = "qemu-se" ] && \
		skip "See: https://github.com/kata-containers/kata-containers/issues/10002"
	get_pod_config_dir
}

@test "Containers with shared volume" {
	pod_name="test-shared-volume"
	first_container_name="busybox-first-container"
	second_container_name="busybox-second-container"
	cmd="cat /tmp/pod-data"
	yaml_file="${pod_config_dir}/pod-shared-volume.yaml"

	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	exec_command=(sh -c "${cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pods
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# Communicate containers
	msg="Hello from the $second_container_name"
	kubectl exec "$pod_name" -c "$first_container_name" -- "${exec_command[@]}" | grep "$msg"
}

@test "initContainer with shared volume" {

	pod_name="initcontainer-shared-volume"
	last_container="last"
	cmd='test $(cat /volume/initContainer) -lt $(cat /volume/container)'
	yaml_file="${pod_config_dir}/initContainer-shared-volume.yaml"

	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"

	exec_command=(sh -c "${cmd}")
	add_exec_to_policy_settings "${policy_settings_dir}" "${exec_command[@]}"

	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pods
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	kubectl exec "$pod_name" -c "$last_container" -- "${exec_command[@]}"
}

teardown() {
	[ "${KATA_HYPERVISOR}" = "qemu-se" ] && \
		skip "See: https://github.com/kata-containers/kata-containers/issues/10002"
	# Debugging information
	kubectl describe "pod/$pod_name" || true

	kubectl delete pod "$pod_name" || true
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}

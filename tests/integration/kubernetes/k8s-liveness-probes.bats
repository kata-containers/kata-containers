#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/lib.sh"
load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	sleep_liveness=20
	agnhost_name="${container_images_agnhost_name}"
	agnhost_version="${container_images_agnhost_version}"

	setup_common || die "setup_common failed"
	get_pod_config_dir
}

@test "Liveness probe" {
	pod_name="liveness-exec"

	yaml_file="${pod_config_dir}/probe-pod-liveness.yaml"
	cp "${pod_config_dir}/pod-liveness.yaml" "${yaml_file}"
	set_node "${yaml_file}" "$node"
	add_allow_all_policy_to_yaml "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check liveness probe returns a success code
	kubectl describe pod "$pod_name" | grep -E "Liveness|#success=1"

	# Sleep necessary to check liveness probe returns a failure code
	sleep "$sleep_liveness"
	kubectl describe pod "$pod_name" | grep "Liveness probe failed"
}

@test "Liveness http probe" {
	pod_name="liveness-http"

	# Create pod specification.
	yaml_file="${pod_config_dir}/http-pod-liveness.yaml"

	sed -e "s#\${agnhost_image}#${agnhost_name}:${agnhost_version}#" \
		"${pod_config_dir}/pod-http-liveness.yaml" > "${yaml_file}"
	set_node "${yaml_file}" "$node"
	add_allow_all_policy_to_yaml "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check liveness probe returns a success code
	kubectl describe pod "$pod_name" | grep -E "Liveness|#success=1"

	# Sleep necessary to check liveness probe returns a failure code
	sleep "$sleep_liveness"
	kubectl describe pod "$pod_name" | grep "Started container"
}


@test "Liveness tcp probe" {
	pod_name="tcptest"

	# Create pod specification.
	yaml_file="${pod_config_dir}/tcp-pod-liveness.yaml"

	sed -e "s#\${agnhost_image}#${agnhost_name}:${agnhost_version}#" \
		"${pod_config_dir}/pod-tcp-liveness.yaml" > "${yaml_file}"
	set_node "${yaml_file}" "$node"
	add_allow_all_policy_to_yaml "${yaml_file}"

	# Create pod
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod "$pod_name"

	# Check liveness probe returns a success code
	kubectl describe pod "$pod_name" | grep -E "Liveness|#success=1"

	# Sleep necessary to check liveness probe returns a failure code
	sleep "$sleep_liveness"
	kubectl describe pod "$pod_name" | grep "Started container"
}

teardown() {
	# Debugging information
	rm -f "${yaml_file}"

	teardown_common "${node}" "${node_start_time:-}"
}

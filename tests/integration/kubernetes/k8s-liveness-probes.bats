#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	sleep_liveness=20
	agnhost_name="${container_images_agnhost_name}"
	agnhost_version="${container_images_agnhost_version}"

	get_pod_config_dir
}

@test "Liveness probe" {
	pod_name="liveness-exec"
	yaml_file="${pod_config_dir}/pod-liveness.yaml"

	auto_generate_policy "" "${yaml_file}"

	# Create pod
	kubectl create -f "${pod_config_dir}/pod-liveness.yaml"

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
	yaml_file="${pod_config_dir}/pod-http-liveness-test.yaml"

	sed -e "s#\${agnhost_image}#${agnhost_name}:${agnhost_version}#" \
		"${pod_config_dir}/pod-http-liveness.yaml" > "${yaml_file}"

	auto_generate_policy "" "${yaml_file}"

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
	yaml_file="${pod_config_dir}/pod-tcp-liveness-test.yaml"

	sed -e "s#\${agnhost_image}#${agnhost_name}:${agnhost_version}#" \
		"${pod_config_dir}/pod-tcp-liveness.yaml" > "${yaml_file}"

	auto_generate_policy "" "${yaml_file}"

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
	kubectl describe "pod/$pod_name"

	kubectl delete pod "$pod_name"
	rm -f ${pod_config_dir}/pod-http-liveness-test.yaml
	rm -f ${pod_config_dir}/pod-tcp-liveness-test.yaml
}

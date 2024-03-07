#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	nginx_version="${docker_images_nginx_version}"
	nginx_image="nginx:$nginx_version"
	replicas="3"
	deployment="nginx-deployment"
	get_pod_config_dir

	# Create the yaml file
	test_yaml="${pod_config_dir}/test-${deployment}.yaml"
	sed -e "s/\${nginx_version}/${nginx_image}/" \
		"${pod_config_dir}/${deployment}.yaml" > "${test_yaml}"

	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${test_yaml}"
}

@test "Scale nginx deployment" {
	kubectl create -f "${test_yaml}"
	kubectl wait --for=condition=Available --timeout=$timeout deployment/${deployment}
	kubectl expose deployment/${deployment}
	kubectl scale deployment/${deployment} --replicas=${replicas}
	cmd="kubectl get deployment/${deployment} -o yaml | grep 'availableReplicas: ${replicas}'"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
}

teardown() {
	rm -f "${test_yaml}"
	kubectl delete deployment "$deployment"
	kubectl delete service "$deployment"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}

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

	get_pod_config_dir

	# Create yaml
	test_yaml="${pod_config_dir}/test-replication-controller.yaml"
	sed -e "s/\${nginx_version}/${nginx_image}/" \
		"${pod_config_dir}/replication-controller.yaml" > "${test_yaml}"

	# Add policy to the yaml file
	policy_settings_dir="$(create_tmp_policy_settings_dir "${pod_config_dir}")"
	add_requests_to_policy_settings "${policy_settings_dir}" "ReadStreamRequest"
	auto_generate_policy "${policy_settings_dir}" "${test_yaml}"
}

@test "Replication controller" {
	replication_name="replicationtest"

	# Create replication controller
	kubectl create -f "${test_yaml}"

	# Check replication controller
	local cmd="kubectl describe replicationcontrollers/$replication_name | grep replication-controller"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	number_of_replicas=$(kubectl get replicationcontrollers/"$replication_name" \
		--output=jsonpath='{.spec.replicas}')
	[ "${number_of_replicas}" -gt 0 ]

	# The replicas pods can be in running, waiting, succeeded or failed
	# status. We need them all on running state before proceed.
	cmd="kubectl describe rc/\"${replication_name}\""
	cmd+="| grep \"Pods Status\" | grep \"${number_of_replicas} Running\""
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Check number of pods created for the
	# replication controller is equal to the
	# number of replicas that we defined
	launched_pods=($(kubectl get pods --selector=app=nginx-rc-test \
		--output=jsonpath={.items..metadata.name}))
	[ "${#launched_pods[@]}" -eq "$number_of_replicas" ]

	# Check pod creation
	for pod_name in ${launched_pods[@]}; do
		cmd="kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name"
		waitForProcess "$wait_time" "$sleep_time" "$cmd"
	done
}

teardown() {
	# Debugging information
	kubectl describe replicationcontrollers/"$replication_name"

	rm -f "${test_yaml}"
	kubectl delete rc "$replication_name"
	delete_tmp_policy_settings_dir "${policy_settings_dir}"
}

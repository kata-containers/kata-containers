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
	yaml_file="${pod_config_dir}/test-replication-controller.yaml"
}

@test "Replication controller" {
	replication_name="replicationtest"

	# Create yaml
	sed -e "s/\${nginx_version}/${nginx_image}/" \
		"${pod_config_dir}/replication-controller.yaml" > "${yaml_file}"

	auto_generate_policy "" "${yaml_file}"

	# Create replication controller
	kubectl create -f "${yaml_file}"

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

	rm -f "${yaml_file}"
	kubectl delete rc "$replication_name"
}

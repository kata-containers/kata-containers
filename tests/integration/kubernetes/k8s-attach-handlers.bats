#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	nginx_version="${docker_images_nginx_version}"
	nginx_image="nginx:$nginx_version"

	pod_name="handlers"

	get_pod_config_dir
	yaml_file="${pod_config_dir}/test-lifecycle-events.yaml"
}

@test "Running with postStart and preStop handlers" {
	# Create yaml
	sed -e "s/\${nginx_version}/${nginx_image}/" \
		"${pod_config_dir}/lifecycle-events.yaml" > "${yaml_file}"

	# TODO: disabled due to #8850
	# auto_generate_policy "${yaml_file}"

	# Create the pod with postStart and preStop handlers
	kubectl create -f "${yaml_file}"

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# Check postStart message
	display_message="cat /usr/share/message"
	check_postStart=$(kubectl exec $pod_name -- sh -c "$display_message" | grep "Hello from the postStart handler")
}

teardown(){
	# Debugging information
	kubectl describe "pod/$pod_name"

	rm -f "${yaml_file}"
	kubectl delete pod "$pod_name"
}

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
}

@test "Scale nginx deployment" {
	sed -e "s/\${nginx_version}/${nginx_image}/" \
		"${pod_config_dir}/${deployment}.yaml" > "${pod_config_dir}/test-${deployment}.yaml"

	kubectl create -f "${pod_config_dir}/test-${deployment}.yaml"
	kubectl wait --for=condition=Available --timeout=$timeout deployment/${deployment}
	kubectl expose deployment/${deployment}
	kubectl scale deployment/${deployment} --replicas=${replicas}
	cmd="kubectl get deployment/${deployment} -o yaml | grep 'availableReplicas: ${replicas}'"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
}

teardown() {
	rm -f "${pod_config_dir}/test-${deployment}.yaml"
	kubectl delete deployment "$deployment"
	kubectl delete service "$deployment"
}

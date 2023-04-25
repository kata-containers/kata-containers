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
	busybox_image="busybox"
	deployment="nginx-deployment"

	get_pod_config_dir
}

@test "Verify nginx connectivity between pods" {

	# Create test .yaml
	sed -e "s/\${nginx_version}/${nginx_image}/" \
		"${pod_config_dir}/${deployment}.yaml" > "${pod_config_dir}/test-${deployment}.yaml"

	kubectl create -f "${pod_config_dir}/test-${deployment}.yaml"
	kubectl wait --for=condition=Available --timeout=$timeout deployment/${deployment}
	kubectl expose deployment/${deployment}

	busybox_pod="test-nginx"
	kubectl run $busybox_pod --restart=Never -it --image="$busybox_image" \
		-- sh -c 'i=1; while [ $i -le '"$wait_time"' ]; do wget --timeout=5 '"$deployment"' && break; sleep 1; i=$(expr $i + 1); done'

	# check pod's status, it should be Succeeded.
	# or {.status.containerStatuses[0].state.terminated.reason} = "Completed"
	[ $(kubectl get pods/$busybox_pod -o jsonpath="{.status.phase}") = "Succeeded" ]
	kubectl logs "$busybox_pod" | grep "index.html"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$busybox_pod"
	kubectl get "pod/$busybox_pod" -o yaml
	kubectl logs "$busybox_pod"
	kubectl get deployment/${deployment} -o yaml
	kubectl get service/${deployment} -o yaml
	kubectl get endpoints/${deployment} -o yaml

	rm -f "${pod_config_dir}/test-${deployment}.yaml"
	kubectl delete deployment "$deployment"
	kubectl delete service "$deployment"
	kubectl delete pod "$busybox_pod"
}

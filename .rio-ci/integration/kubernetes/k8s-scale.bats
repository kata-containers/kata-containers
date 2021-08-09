#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	replicas="3"
	deployment="nginx-deployment"
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	get_pod_config_dir
}

@test "Scale nginx deployment" {
	pcl	"${pod_config_dir}/deployment-nginx.pcl" | kubectl create -f -

	kubectl wait --for=condition=Available --timeout=$timeout deployment/${deployment}
	kubectl expose deployment/${deployment}
	kubectl scale deployment/${deployment} --replicas=${replicas}
	cmd="kubectl get deployment/${deployment} -o yaml | grep 'availableReplicas: ${replicas}'"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
}

teardown() {
	kubectl delete deployment "$deployment"
	kubectl delete service "$deployment"
}

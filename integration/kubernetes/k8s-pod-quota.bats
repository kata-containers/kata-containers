#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	if kubectl get runtimeclass | grep kata; then
		pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
	else
		pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
	fi
}

@test "Pod quota" {
	resource_name="pod-quota"
	deployment_name="deploymenttest"
	wait_time=10
	sleep_time=2

	# Create the resourcequota
	kubectl create -f "${pod_config_dir}/resource-quota.yaml"

	# View information about resourcequota
	kubectl get resourcequota "$resource_name" --output=yaml | grep 'pods: "2"'

	# Create deployment
	kubectl create -f "${pod_config_dir}/pod-quota-deployment.yaml"

	# View information about the deployment
	cmd="kubectl get deployment \"$deployment_name\" --output=yaml | grep -q 'forbidden: exceeded quota'"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
}

teardown() {
	kubectl delete resourcequota "$resource_name"
	kubectl delete deployment "$deployment_name"
}

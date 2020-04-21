#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	get_pod_config_dir
}

@test "Pod quota" {
	resource_name="pod-quota"
	deployment_name="deploymenttest"

	# Create the resourcequota
	kubectl create -f "${pod_config_dir}/resource-quota.yaml"

	# View information about resourcequota
	kubectl get resourcequota "$resource_name" --output=yaml | grep 'pods: "2"'

	# Create deployment
	kubectl create -f "${pod_config_dir}/pod-quota-deployment.yaml"

	# View deployment
	kubectl wait --for=condition=Available deployment/${deployment_name}
}

teardown() {
	kubectl delete resourcequota "$resource_name"
	kubectl delete deployment "$deployment_name"
}

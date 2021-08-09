#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	get_pod_config_dir
}

@test "Pod quota" {
	resource_name="pod-quota"
	deployment_name="deploymenttest"

	# Create the resourcequota
	pcl "${pod_config_dir}/resource-quota.pcl" | kubectl create -f -

	# View information about resourcequota
	kubectl get resourcequota "$resource_name" --output=yaml | grep 'pods: "2"'

	# Create deployment
	pcl -e APPNAME="${deployment_name}" "${pod_config_dir}/deployment-pod-quota.pcl" | kubectl create -f -

	# View deployment
	kubectl wait --for=condition=Available --timeout=$timeout deployment/${deployment_name}
}

teardown() {
	kubectl delete resourcequota "$resource_name"
	kubectl delete deployment "$deployment_name"
}

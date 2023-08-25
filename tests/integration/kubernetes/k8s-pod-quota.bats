#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir
}

@test "Pod quota" {
	resource_name="pod-quota"
	deployment_name="deploymenttest"

	# Create the resourcequota
	kubectl create -f "${pod_config_dir}/resource-quota.yaml"

	# View information about resourcequota
	kubectl get resourcequota "$resource_name" \
		--output=yaml | grep 'pods: "2"'

	# Create deployment
	kubectl create -f "${pod_config_dir}/pod-quota-deployment.yaml"

	# View deployment
	kubectl wait --for=condition=Available --timeout=$timeout \
		deployment/${deployment_name}
}

teardown() {
	kubectl delete -f "${pod_config_dir}/pod-quota-deployment.yaml"
	kubectl delete -f "${pod_config_dir}/resource-quota.yaml"
}

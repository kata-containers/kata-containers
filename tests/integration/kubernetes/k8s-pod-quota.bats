#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: https://github.com/kata-containers/kata-containers/issues/7873"

	get_pod_config_dir
	deployment_yaml_file="${pod_config_dir}/pod-quota-deployment.yaml"
}

@test "Pod quota" {
	resource_name="pod-quota"
	deployment_name="deploymenttest"

	auto_generate_policy "" "${deployment_yaml_file}"

	# Create the resourcequota
	kubectl create -f "${pod_config_dir}/resource-quota.yaml"

	# View information about resourcequota
	kubectl get resourcequota "$resource_name" \
		--output=yaml | grep 'pods: "2"'

	# Create deployment
	kubectl create -f "${deployment_yaml_file}"

	# View deployment
	kubectl wait --for=condition=Available --timeout=$timeout \
		deployment/${deployment_name}
}

teardown() {
	[ "${KATA_HYPERVISOR}" == "fc" ] && skip "test not working see: https://github.com/kata-containers/kata-containers/issues/7873"

	# Debugging information
	kubectl describe deployment ${deployment_name}

	# Clean-up
	kubectl delete -f "${deployment_yaml_file}"
	kubectl delete -f "${pod_config_dir}/resource-quota.yaml"
}

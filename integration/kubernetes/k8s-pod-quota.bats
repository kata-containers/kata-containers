#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	if sudo -E kubectl get runtimeclass | grep kata; then
		pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
	else
		pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
	fi
}

@test "Pod quota" {
	resource_name="pod-quota"
	deployment_name="deploymenttest"

	# Create the resourcequota
	sudo -E kubectl create -f "${pod_config_dir}/resource-quota.yaml"

	# View information about resourcequota
	sudo -E kubectl get resourcequota "$resource_name" --output=yaml | grep 'pods: "2"'

	# Create deployment
	sudo -E kubectl create -f "${pod_config_dir}/pod-quota-deployment.yaml"

	# View information about the deployment
	sudo -E kubectl get deployment "$deployment_name" --output=yaml | grep "forbidden: exceeded quota"
}

teardown() {
	sudo -E kubectl delete resourcequota "$resource_name"
	sudo -E kubectl delete deployment "$deployment_name"
}

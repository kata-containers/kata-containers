#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	export KUBECONFIG=/etc/kubernetes/admin.conf
	if sudo -E kubectl get runtimeclass | grep -q kata; then
		pod_config_dir="${BATS_TEST_DIRNAME}/runtimeclass_workloads"
	else
		pod_config_dir="${BATS_TEST_DIRNAME}/untrusted_workloads"
	fi
}

@test "ConfigMap for a pod" {
	config_name="test-configmap"
	pod_name="config-env-test-pod"

	# Create ConfigMap
	sudo -E kubectl create -f "${pod_config_dir}/configmap.yaml"

	# View the values of the keys
	sudo -E kubectl get configmaps $config_name -o yaml | grep -q "data-"

	# Create a pod that consumes the ConfigMap
	sudo -E kubectl create -f "${pod_config_dir}/pod-configmap.yaml"

	# Check pod creation
	sudo -E kubectl wait --for=condition=Ready pod "$pod_name"

	# Check env
	cmd="env"
	sudo -E kubectl exec $pod_name -- sh -c $cmd | grep "KUBE_CONFIG_1=value-1"
	sudo -E kubectl exec $pod_name -- sh -c $cmd | grep "KUBE_CONFIG_2=value-2"
}

teardown() {
	sudo -E kubectl delete pod "$pod_name"
	sudo -E kubectl delete configmap "$config_name"
}

#!/usr/bin/env bats
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

@test "Credentials using secrets" {
	secret_name="test-secret"
	pod_name="secret-test-pod"
	second_pod_name="secret-envars-test-pod"

	# Create the secret
	kubectl create -f "${pod_config_dir}/inject_secret.yaml"

	# View information about the secret
	kubectl get secret "${secret_name}" -o yaml | grep "type: Opaque"

	# Create a pod that has access to the secret through a volume
	kubectl create -f "${pod_config_dir}/pod-secret.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# List the files
	cmd="ls /tmp/secret-volume"
	kubectl exec $pod_name -- sh -c "$cmd" | grep -w "password"
	kubectl exec $pod_name -- sh -c "$cmd" | grep -w "username"

	# Create a pod that has access to the secret data through environment variables
	kubectl create -f "${pod_config_dir}/pod-secret-env.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$second_pod_name"

	# Display environment variables
	second_cmd="printenv"
	kubectl exec $second_pod_name -- sh -c "$second_cmd" | grep -w "SECRET_USERNAME"
	kubectl exec $second_pod_name -- sh -c "$second_cmd" | grep -w "SECRET_PASSWORD"
}

teardown() {
	kubectl delete pod "$pod_name" "$second_pod_name"
	kubectl delete secret "$secret_name"
}

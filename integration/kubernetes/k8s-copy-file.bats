#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	export KUBECONFIG="$HOME/.kube/config"
	pod_name="test-env"
	get_pod_config_dir
	file_name="file.txt"
	content="Hello"
}

@test "Copy file in a pod" {
	# Create pod
	kubectl create -f "${pod_config_dir}/pod-env.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Create a file
	echo "$content" > "$file_name"

	# Copy file into a pod
	kubectl cp "$file_name" $pod_name:/tmp

	# Print environment variables
	kubectl exec $pod_name -- sh -c "cat /tmp/$file_name | grep $content"
}

@test "Copy from pod to host" {
	# Create pod
	kubectl create -f "${pod_config_dir}/pod-env.yaml"

	# Check pod creation
	kubectl wait --for=condition=Ready pod "$pod_name"

	# Create a file in the pod
	kubectl exec "$pod_name" -- sh -c "cd /tmp && echo $content > $file_name"

	# Copy file from pod to host
	kubectl cp "$pod_name":/tmp/"$file_name" "$file_name"

	# Verify content
	cat "$file_name" | grep "$content"
}

teardown() {
	rm -f "$file_name"
	kubectl delete pod "$pod_name"
}

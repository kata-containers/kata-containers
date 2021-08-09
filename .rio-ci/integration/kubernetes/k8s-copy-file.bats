#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	export KUBECONFIG="${KUBECONFIG:-$HOME/.kube/config}"
	get_pod_config_dir
	file_name="file.txt"
	content="Hello"
}

@test "Copy file in a pod" {
	# Create pod
	pod_name="pod-copy-file-from-host"
	pcl -e APPNAME="${pod_name}" "${pod_config_dir}/pod-busybox.pcl" | kubectl create -f -

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	# Create a file
	echo "$content" > "$file_name"

	# Copy file into a pod
	kubectl cp "$file_name" $pod_name:/tmp

	# Print environment variables
	kubectl exec $pod_name -- sh -c "cat /tmp/$file_name | grep $content"
}

@test "Copy from pod to host" {
	# Create pod
	pod_name="pod-copy-file-to-host"
	pcl -e APPNAME="${pod_name}" "${pod_config_dir}/pod-busybox.pcl" | kubectl create -f -

	# Check pod creation
	kubectl wait --for=condition=Ready --timeout=$timeout pod $pod_name

	kubectl logs "$pod_name" || true
	kubectl describe pod "$pod_name" || true
	kubectl get pods --all-namespaces

	# Create a file in the pod
	kubectl exec "$pod_name" -- sh -c "cd /tmp && echo $content > $file_name"

	kubectl logs "$pod_name" || true
	kubectl describe pod "$pod_name" || true
	kubectl get pods --all-namespaces

	# Copy file from pod to host
	kubectl cp "$pod_name":/tmp/"$file_name" "$file_name"

	# Verify content
	cat "$file_name" | grep "$content"
}

teardown() {
	# Debugging information
	kubectl describe "pod/$pod_name"

	rm -f "$file_name"
	kubectl delete pod "$pod_name"
}

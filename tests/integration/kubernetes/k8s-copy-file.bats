#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/tests_common.sh"

setup() {
	get_pod_config_dir
	file_name="file.txt"
	content="Hello"
}

@test "Copy file in a pod" {
	# Create pod
	pod_name="pod-copy-file-from-host"
	ctr_name="ctr-copy-file-from-host"

	pod_config=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	cp "$pod_config_dir/busybox-template.yaml" "$pod_config"
	sed -i "s/POD_NAME/$pod_name/" "$pod_config"
	sed -i "s/CTR_NAME/$ctr_name/" "$pod_config"

	# TODO: disabled due to #8868
	# auto_generate_policy "$pod_config"

	kubectl create -f "${pod_config}"

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
	ctr_name="ctr-copy-file-to-host"

	pod_config=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	cp "$pod_config_dir/busybox-template.yaml" "$pod_config"
	sed -i "s/POD_NAME/$pod_name/" "$pod_config"
	sed -i "s/CTR_NAME/$ctr_name/" "$pod_config"

	# TODO: disabled due to #8868
	# auto_generate_policy "$pod_config"

	kubectl create -f "${pod_config}"

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

	rm -f "$pod_config"
}

#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	busybox_image="busybox"
	export KUBECONFIG="$HOME/.kube/config"
	first_pod_name="first-test"
	second_pod_name="second-test"
	# Pull the images before launching workload.
	sudo -E crictl pull "$busybox_image"

	get_pod_config_dir
	first_pod_config=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	cp "$pod_config_dir/busybox-template.yaml" "$first_pod_config"
	sed -i "s/NAME/${first_pod_name}/" "$first_pod_config"
	second_pod_config=$(mktemp --tmpdir pod_config.XXXXXX.yaml)
	cp "$pod_config_dir/busybox-template.yaml" "$second_pod_config"
	sed -i "s/NAME/${second_pod_name}/" "$second_pod_config"

	uts_cmd="ls -la /proc/self/ns/uts"
	ipc_cmd="ls -la /proc/self/ns/ipc"
}

@test "Check UTS and IPC namespaces" {
	# Run the first pod
	kubectl create -f "$first_pod_config"
	kubectl wait --for=condition=Ready pod "$first_pod_name"
	first_pod_uts_ns=$(kubectl exec "$first_pod_name" -- sh -c "$uts_cmd" | grep uts | cut -d ':' -f3)
	first_pod_ipc_ns=$(kubectl exec "$first_pod_name" -- sh -c "$ipc_cmd" | grep ipc | cut -d ':' -f3)

	# Run the second pod
	kubectl create -f "$second_pod_config"
	kubectl wait --for=condition=Ready pod "$second_pod_name"
	second_pod_uts_ns=$(kubectl exec "$second_pod_name" -- sh -c "$uts_cmd" | grep uts | cut -d ':' -f3)
	second_pod_ipc_ns=$(kubectl exec "$second_pod_name" -- sh -c "$ipc_cmd" | grep ipc | cut -d ':' -f3)

	# Check UTS and IPC namespaces
	[ "$first_pod_uts_ns" == "$second_pod_uts_ns" ]
	[ "$first_pod_ipc_ns" == "$second_pod_ipc_ns" ]
}

teardown() {
	kubectl delete pod "$first_pod_name"
	kubectl delete pod "$second_pod_name"
	rm -rf "$first_pod_config"
	rm -rf "$second_pod_config"
}

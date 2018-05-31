#!/usr/bin/env bats
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

load "${BATS_TEST_DIRNAME}/../../.ci/lib.sh"

setup() {
	busybox_image="busybox"
	export KUBECONFIG=/etc/kubernetes/admin.conf
	first_pod_name="first-test"
	second_pod_name="second-test"
	sleep_cmd="sleep 30"
	# Pull the images before launching workload. This is mainly because we use
	# a timeout and in slow networks it may result in not been able to pull the image
	# successfully.
	sudo -E crictl pull "$busybox_image"
}

@test "Check UTS and IPC namespaces" {
	wait_time=30
	sleep_time=5

	# Run the first pod
	sudo -E kubectl run $first_pod_name --image=$busybox_image -- sh  -c "eval $sleep_cmd"
	first_pod_status_cmd="sudo -E kubectl get pods -a | grep $first_pod_name | grep Running"
	waitForProcess "$wait_time" "$sleep_time" "$first_pod_status_cmd"

	# Run the second pod
	sudo -E kubectl run $second_pod_name --image=$busybox_image -- sh  -c "eval $sleep_cmd"
	second_pod_status_cmd="sudo -E kubectl get pods -a | grep $second_pod_name | grep Running"
	waitForProcess "$wait_time" "$sleep_time" "$second_pod_status_cmd"

	# Check UTS namespace
	uts_cmd="ls -la /proc/self/ns/uts"
	first_complete_pod_name=$(sudo -E kubectl get pods | grep "$first_pod_name" | cut -d ' ' -f1)
	second_complete_pod_name=$(sudo -E kubectl get pods | grep "$second_pod_name" | cut -d ' ' -f1)
	first_pod_uts_namespace=$(sudo -E kubectl exec "$first_complete_pod_name" -- sh -c "$uts_cmd" | grep uts | cut -d ':' -f3)
	second_pod_uts_namespace=$(sudo -E kubectl exec "$second_complete_pod_name" -- sh -c "$uts_cmd" | grep uts | cut -d ':' -f3)
	[ "$first_pod_uts_namespace" == "$second_pod_uts_namespace" ]

	# Check IPC namespace
	ipc_cmd="ls -la /proc/self/ns/ipc"
	first_pod_ipc_namespace=$(sudo -E kubectl exec "$first_complete_pod_name" -- sh -c "$ipc_cmd" | grep ipc | cut -d ':' -f3)
	second_pod_ipc_namespace=$(sudo -E kubectl exec "$second_complete_pod_name" -- sh -c "$ipc_cmd" | grep ipc | cut -d ':' -f3)
	[ "$first_pod_ipc_namespace" == "$second_pod_ipc_namespace" ]
}

teardown() {
	sudo -E kubectl delete deployment "$first_pod_name"
	sudo -E kubectl delete deployment "$second_pod_name"
	# Wait for the pods to be deleted
	cmd="sudo -E kubectl get pods | grep found."
	waitForProcess "$wait_time" "$sleep_time" "$cmd"
	sudo -E kubectl get pods
}

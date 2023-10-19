#!/bin/bash
# Copyright (c) 2021, 2022 IBM Corporation
# Copyright (c) 2022, 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# This provides generic functions to use in the tests.
#
set -e

# Delete all pods if any exist, otherwise just return
#
k8s_delete_all_pods_if_any_exists() {
	[ -z "$(kubectl get --no-headers pods)" ] || \
		kubectl delete --all pods
}

# Wait until the pod is not 'Ready'. Fail if it hits the timeout.
#
# Parameters:
#	$1 - the sandbox ID
#	$2 - wait time in seconds. Defaults to 120. (optional)
#
k8s_wait_pod_be_ready() {
	local pod_name="$1"
	local wait_time="${2:-120}"

	kubectl wait --timeout="${wait_time}s" --for=condition=ready "pods/$pod_name"
}

# Create a pod and wait it be ready, otherwise fail.
#
# Parameters:
#	$1 - the pod configuration file.
#
k8s_create_pod() {
	local config_file="$1"
	local pod_name=""

	if [ ! -f "${config_file}" ]; then
		echo "Pod config file '${config_file}' does not exist"
		return 1
	fi

	kubectl apply -f "${config_file}"
	if ! pod_name=$(kubectl get pods -o jsonpath='{.items..metadata.name}'); then
		echo "Failed to create the pod"
		return 1
	fi

	if ! k8s_wait_pod_be_ready "$pod_name"; then
		# TODO: run this command for debugging. Maybe it should be
		#       guarded by DEBUG=true?
		kubectl get pods "$pod_name"
		return 1
	fi
}

# Create a pod then assert it fails to run. Use in tests that you expect the
# pod creation to fail.
#
# Note: a good testing practice is to afterwards check that the pod creation
# failed because of the expected reason.
#
# Parameters:
#	$1 - the pod configuration file.
#
assert_pod_fail() {
	local container_config="$1"
	echo "In assert_pod_fail: $container_config"

	echo "Attempt to create the container but it should fail"
	! k8s_create_pod "$container_config" || /bin/false
}
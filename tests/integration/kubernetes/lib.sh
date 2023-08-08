#!/bin/bash
# Copyright (c) 2021, 2022 IBM Corporation
# Copyright (c) 2022, 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# This provides generic functions to use in the tests.
#

# Wait until the pod is not 'Ready'. Fail if it hits the timeout.
#
# Parameters:
#	$1 - the pod name
#	$2 - wait time in seconds. Defaults to 90. (optional)
#
wait_pod_to_be_ready() {
	local pod_name="$1"
	local wait_time="${2:-90}"

	kubectl wait --timeout="${wait_time}s" --for=condition=ready "pods/$pod_name"
}

# Create a pod and wait it to be ready, otherwise fail.
#
# Parameters:
#	$1 - the pod configuration file.
#	$2 - the pod name (optional)
#
create_pod_and_wait() {
	local config_file="$1"
	local pod_name="${2:-}"

	if [ ! -f "${config_file}" ]; then
		echo "Pod config file '${config_file}' does not exist"
		return 1
	fi

	if ! kubectl apply -f "${config_file}"; then
		echo "Failed to create the pod"
		return 1
	fi

	pod_name=${pod_name:-$(kubectl get pods -o jsonpath='{.items..metadata.name}')}
	if [ -z "$pod_name" ]; then
		echo "Unable to get the pod name"
		return 1
	fi

	if ! wait_pod_to_be_ready "$pod_name"; then
		# TODO: run this command for debugging. Maybe it should be
		#       guarded by DEBUG=true?
		kubectl get pods "$pod_name"
		return 1
	fi
}
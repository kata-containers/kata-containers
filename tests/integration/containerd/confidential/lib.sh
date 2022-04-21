#!/bin/bash
# Copyright (c) 2021, 2022 IBM Corporation
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# This provides generic functions to use in the tests.
#
set -e

source "${BATS_TEST_DIRNAME}/../../../lib/common.bash"
FIXTURES_DIR="${BATS_TEST_DIRNAME}/fixtures"

# Delete the containers alongside the Pod.
#
# Parameters:
#	$1 - the sandbox name
#
crictl_delete_cc_pod() {
	local sandbox_name="$1"
	local pod_id="$(sudo crictl pods --name ${sandbox_name} -q)"
	local container_ids="$(sudo crictl ps --pod ${pod_id} -q)"

	if [ -n "${container_ids}" ]; then
		while read -r container_id; do
			sudo crictl stop "${container_id}"
			sudo crictl rm "${container_id}"
		done <<< "${container_ids}"
	fi
	sudo crictl stopp "${pod_id}"
	sudo crictl rmp "${pod_id}"
}

# Delete the pod if it exists, otherwise just return.
#
# Parameters:
#	$1 - the sandbox name
#
crictl_delete_cc_pod_if_exists() {
	local sandbox_name="$1"

	[ -z "$(sudo crictl pods --name ${sandbox_name} -q)" ] || \
		crictl_delete_cc_pod "${sandbox_name}"
}

# Wait until the pod is not 'Ready'. Fail if it hits the timeout.
#
# Parameters:
#	$1 - the sandbox ID
#	$2 - wait time in seconds. Defaults to 10. (optional)
#	$3 - sleep time in seconds between checks. Defaults to 5. (optional)
#
crictl_wait_cc_pod_be_ready() {
	local pod_id="$1"
	local wait_time="${2:-10}"
	local sleep_time="${3:-5}"

	local cmd="[ \$(sudo crictl pods --id $pod_id -q --state ready |\
	       	wc -l) -eq 1 ]"
	if ! waitForProcess "$wait_time" "$sleep_time" "$cmd"; then
		echo "Pod ${pod_id} not ready after ${wait_time}s"
		return 1
	fi
}

# Create a pod and wait it be ready, otherwise fail.
#
# Parameters:
#	$1 - the pod configuration file.
#
crictl_create_cc_pod() {
	local config_file="$1"
	local pod_id=""

	if [ ! -f "$config_file" ]; then
		echo "Pod config file '${config_file}' does not exist"
		return 1
	fi

	if ! pod_id=$(sudo crictl runp -r kata "$config_file"); then
		echo "Failed to create the pod"
		return 1
	fi

	if ! crictl_wait_cc_pod_be_ready "$pod_id"; then
		# TODO: run this command for debugging. Maybe it should be
		#       guarded by DEBUG=true?
		sudo crictl pods
		return 1
	fi
}

# Wait until the container does not start running. Fail if it hits the timeout.
#
# Parameters:
#	$1 - the container ID.
#	$2 - wait time in seconds. Defaults to 30. (optional)
#	$3 - sleep time in seconds between checks. Defaults to 10. (optional)
#
crictl_wait_cc_container_be_running() {
	local container_id="$1"
	local wait_time="${2:-30}"
	local sleep_time="${3:-10}"

	local cmd="[ \$(sudo crictl ps --id $container_id -q --state running | \
		wc -l) -eq 1 ]"
	if ! waitForProcess "$wait_time" "$sleep_time" "$cmd"; then
		echo "Container $container_id is not running after ${wait_time}s"
		return 1
	fi
}

# Create a container and wait it be running.
#
# Parameters:
#	$1 - the pod name.
#	$2 - the pod configuration file.
#	$3 - the container configuration file.
#
crictl_create_cc_container() {
	local pod_name="$1"
	local pod_config="$2"
	local container_config="$3"
	local container_id=""
	local pod_id=""

	if [[ ! -f "$pod_config" || ! -f "$container_config" ]]; then
		echo "Pod or container config file does not exist"
		return 1
	fi

	pod_id=$(sudo crictl pods --name ${pod_name} -q)
	container_id=$(sudo crictl create -with-pull "${pod_id}" \
		"${container_config}" "${pod_config}")

	if [ -z "$container_id" ]; then
		echo "Failed to create the container"
		return 1
	fi

	if ! sudo crictl start ${container_id}; then
		echo "Failed to start container $container_id"
		sudo crictl ps -a
		return 1
	fi

	if ! crictl_wait_cc_container_be_running "$container_id"; then
		sudo crictl ps -a
		return 1
	fi
}

#!/bin/bash
# Copyright (c) 2021, 2022 IBM Corporation
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# This provides generic assert functions to use in the tests.
#

# Create container and check it is operational.
#
# Parameters:
#	$1 - the container configuration file.
#
# Note: the global $sandbox_name should be set already.
#
assert_container() {
	local container_config="$1"

	echo "Create the cc container"
	crictl_create_cc_container "$sandbox_name" "$pod_config" \
		"$container_config"

	echo "Check the container is operational"
	local pod_id=$(crictl pods --name "$sandbox_name" -q)
	local container_id=$(crictl ps --pod ${pod_id} -q)
	crictl exec "$container_id" cat /proc/cmdline
}

# Try to create a container when it is expected to fail.
#
# Parameters:
#	$1 - the container configuration file.
#
# Note: the global $sandbox_name and $pod_config should be set already.
#
assert_container_fail() {
	local container_config="$1"

	echo "Attempt to create the container but it should fail"
	! crictl_create_cc_container "$sandbox_name" "$pod_config" \
		"$container_config" || /bin/false
}

# Check the logged messages on host have a given message.
#
# Parameters:
#	$1 - the message
#
# Note: get the logs since the global $start_date.
#
assert_logs_contain() {
	local message="$1"
	journalctl -x -t kata --since "$start_date" | grep "$message"
}


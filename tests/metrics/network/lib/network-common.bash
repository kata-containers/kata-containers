#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Description: This file contains functions that are shared among the networking
# tests that are using nuttcp and iperf3

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

# This function will launch a container in detached mode and
# it will return the IP address, the role of this container is as a server.
# Arguments:
#  Docker image.
#  Command[s] to be executed.
#  Extra argument for container execution.
function start_server()
{
	local image="$1"
	local cmd="$2"
	local extra_args="$3"

	# Launch container
	instance_id="$(docker run $extra_args -d --runtime "$RUNTIME" \
		"$image" sh -c "$cmd")"

	# Get IP Address
	server_address=$(docker inspect --format "{{.NetworkSettings.IPAddress}}" $instance_id)

	echo "$server_address"
}

# This function will launch a container and it will execute a determined
# workload, this workload is received as an argument and this function will
# return the output/result of the workload. The role of this container is as a client.
# Arguments:
#  Docker image
#  Command[s] to be executed
#  Extra argument for container execution
function start_client()
{
	local image="$1"
	local cmd="$2"
	local extra_args="$3"

	# Execute client/workload and return result output
	output="$(docker run $extra_args --runtime "$RUNTIME" \
		"$image" sh -c "$cmd")"

	echo "$output"
}

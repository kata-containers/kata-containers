#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This test measures bidirectional bandwidth. This
# is used when we want to test both directions for the
# maximum amount of throughput.
# Bidirectional bandwidth is only supported by iperf,
# this feature was dropped for iperf3.

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/lib/network-common.bash"
source "${SCRIPT_PATH}/../lib/common.bash"

# Test name
TEST_NAME="${TEST_NAME:-network iperf bidirectional bandwidth}"
# Image name
image="${image:-local-iperf}"
# Dockerfile
dockerfile="${SCRIPT_PATH}/iperf_dockerfile/Dockerfile"
# Measurement time (seconds)
transmit_timeout="${transmit_timeout:-30}"

save_config(){
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"image": "$image",
		"iperf version": "2.0.5",
		"transmit timeout": $transmit_timeout
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"
}


function main() {
	cmds=("awk")
	check_cmds "${cmds[@]}"

	# Check no processes are left behind
	check_processes
	check_dockerfiles_images "$image" "$dockerfile"
	init_env

	# Start iperf server configuration
	# Set the TMPDIR to an existing tmpfs mount to avoid a 9p unlink error
	local init_cmds="export TMPDIR=/dev/shm"
	local server_command="$init_cmds && iperf -s"
	local server_address=$(start_server "$image" "$server_command" "$server_extra_args")

	# Verify server IP address
	if [ -z "$server_address" ];then
		clean_env
		die "server: ip address no found"
	fi

	# Start iperf client
	local client_command="$init_cmds && iperf -c $server_address -d -T $transmit_timeout"

	metrics_json_init
	save_config

	result=$(start_client "$image" "$client_command" "$client_extra_args")

	metrics_json_start_array

	local total_bidirectional_client_bandwidth=$(echo $result | tail -1 | awk '{print $(NF-9)}')
	local total_bidirectional_client_bandwidth_units=$(echo $result | tail -1 | awk '{print $(NF-8)}')
	local total_bidirectional_server_bandwidth=$(echo $result | tail -1 |  awk '{print $(NF-1)}')
	local total_bidirectional_server_bandwidth_units=$(echo $result | tail -1 |  awk '{print $(NF)}')

	local json="$(cat << EOF
	{
		"client to server": {
			"Result" : $total_bidirectional_client_bandwidth,
			"Units"  : "$total_bidirectional_client_bandwidth_units"
	},
		"server to client": {
			"Result" : $total_bidirectional_server_bandwidth,
			"Units"  : "$total_bidirectional_server_bandwidth_units"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save
	clean_env

	# Check no processes are left behind
	check_processes
}
main "$@"

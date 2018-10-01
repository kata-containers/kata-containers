#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description:
# This metrics test measures the latency spent when a container makes a ping
# to another container.
# container-client <--- ping ---> container-server

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/lib/network-common.bash"
source "${SCRIPT_PATH}/../lib/common.bash"

# Test name
TEST_NAME="${TEST_NAME:-network ping latency}"
# Image name (ping installed by default)
image="${image:-busybox}"
# Number of packets (sent)
number="${number:-30}"

save_config(){
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"image": "$image",
		"number of packets (sent)": $number
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"
}

function main() {
	# Check no processes are left behind
	check_processes

	# Initialize/clean environment
	init_env
	check_images "$image"

	local server_command="tail -f /dev/null"
	local server_address=$(start_server "$image" "$server_command" "$server_extra_args")

	# Verify server IP address
	if [ -z "$server_address" ];then
		clean_env
		die "server: ip address no found"
	fi

	local client_command="ping -c ${number} ${server_address}"

	metrics_json_init
	save_config

	result=$(start_client "$image" "$client_command" "$client_extra_args")

	metrics_json_start_array

	local latency_average="$(echo "$result" | grep "avg" | awk -F"/" '{print $4}')"

	local json="$(cat << EOF
	{
		"latency": {
			"Result" : $latency_average,
			"Units"  : "ms"
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

#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description:
# This metrics test measures Proportional Set Size, Resident Set Size,
# and Virtual Set Size memory while an interconnection
# between container-client <----> container-server transfers 1 Gb rate as a
# network workload using nuttcp.

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"
source "${SCRIPT_PATH}/lib/network-common.bash"

# Test name
TEST_NAME="${TEST_NAME:-network memory}"
# Image name
image="${IMAGE:-local-nuttcp}"
# Dockerfile
dockerfile="${SCRIPT_PATH}/nuttcp_dockerfile/Dockerfile"
# Time for the test to run (seconds)
total_time="${total_time:-30}"
# Time in which we sample PSS, RSS, and VSS (seconds)
settle_time="${settle_time:-15}"
# Rate limit (speed at which transmitter send data, megabytes)
rate_limit="${rate_limit:-1000}"
# Name of the server container
server_name="${server_name:-network-server}"

function save_config {
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"image" : "$image",
		"total time" : "$total_time",
		"settle time" : "$settle_time",
		"rate limit" : "$rate_limit"
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"
}

function main() {
	# Check dependencies
	cmds=("smem" "awk")

	# Check no processes are left behind
	check_processes

	init_env
	check_cmds "${cmds[@]}"
	check_dockerfiles_images "$image" "$dockerfile"

	# Arguments to run the client/server
	local server_extra_args="--name=$server_name"
	local client_extra_args="--rm"

	local server_command="tail -f /dev/null"
	local server_address=$(start_server "$image" "$server_command" "$server_extra_args")

	# Verify server IP address
	if [ -z "$server_address" ];then
		clean_env
		die "server: ip address no found"
	fi

	metrics_json_init
	save_config

	local client_command="/root/nuttcp -R${rate_limit}m -T${total_time} ${server_address}"
	local server_command="/root/nuttcp -S"

	# Execute nuttcp workload in container server
	docker exec ${server_name} sh -c "${server_command}"
	start_client "$image" "$client_command" "$client_extra_args" > /dev/null

	# Time when we are taking our PSS, RSS, and VSS measurement
	echo >&2 "WARNING: sleeping for $settle_time seconds in order to sample the PSS, RSS, and VSS"
	sleep ${settle_time}

	metrics_json_start_array

	# Determine the process that will be measured (PSS, RSS, and VSS memory consumption)
	local process="${HYPERVISOR_PATH}"

	local vss_memory_command="sudo smem --no-header -c vss"
	local vss_result=$(${vss_memory_command} -P ^${process} | awk '{ total += $1 } END { print total/NR }')

	local rss_memory_command="sudo smem --no-header -c rss"
	local rss_result=$(${rss_memory_command} -P ^${process} | awk '{ total += $1 } END { print total/NR }')

	local pss_memory_command="sudo smem --no-header -c pss"
	local pss_result=$(${pss_memory_command} -P ^${process} | awk '{ total += $1 } END { print total/NR }')

	local json="$(cat << EOF
	{
		"PSS network memory": {
			"Result" : $pss_result,
			"Units"  : "Kb"
		},
		"RSS network memory": {
			"Result" : $rss_result,
			"Units"  : "Kb"
		},
		"VSS network memory": {
			"Result" : $vss_result,
			"Units"  : "Kb"
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

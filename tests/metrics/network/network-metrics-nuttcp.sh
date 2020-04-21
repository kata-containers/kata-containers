#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description:
# This metrics test measures the UDP network bandwidth using nuttcp
# in a interconnection container-client <----> container-server.

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/lib/network-common.bash"
source "${SCRIPT_PATH}/../lib/common.bash"

# Measurement time (seconds)
transmit_timeout="${transmit_timeout:-30}"
# Image name
image="${IMAGE:-local-nuttcp}"
# Dockerfile
dockerfile="${SCRIPT_PATH}/nuttcp_dockerfile/Dockerfile"
# Name of the server container
server_name="${server_name:-network-server}"
# Nuttcp version
nuttcp_version="${nuttcp_version:-7.3.2}"

function udp_default_buffer_size {
	# Test UDP Jumbo (9000 byte MTU) packets
	# Packet header (ICMP+IP) is 28bytes, so maximum payload is 8972 bytes
	# See the nuttcp documentation for more hints:
	# https://fasterdata.es.net/performance-testing/network-troubleshooting-tools/nuttcp/
	local bl=8972
	local TEST_NAME="${TEST_NAME:-nuttcp test with ${bl} buffer size}"
	local result="$(udp_bandwidth)"
	save_results "$TEST_NAME" "$result"
}

function udp_specific_buffer_size {
	# Test UDP standard (1500 byte MTU) packets
	# Even though the packet header (ICMP+IP) is 28 bytes, which would
	# result in 1472 byte frames, the nuttcp documentation recommends
	# use of 1200 byte payloads.
	# See the nuttcp examples.txt for more information:
	# http://nuttcp.net/nuttcp/5.1.3/examples.txt
	local bl=1200
 	local TEST_NAME="${TEST_NAME:-nuttcp test with ${bl} buffer size}"
	local result="$(udp_bandwidth)"
	save_results "$TEST_NAME" "$result"
}

function udp_bandwidth {
	# Arguments to run the client/server
	local server_extra_args="--name=$server_name"
	local client_extra_args="--rm"

	local server_command="tail -f /dev/null"
	local server_address=$(start_server "$image" "$server_command" "$server_extra_args")

	local client_command="/root/nuttcp -T${transmit_timeout} -u -Ru -i1 -l${bl} ${server_address}"
	local server_command="/root/nuttcp -u -S"

	docker exec ${server_name} sh -c "${server_command}"
	output=$(start_client "$image" "$client_command" "$client_extra_args")

	clean_env
	echo "$output"

	# Check no processes are left behind
	check_processes
}

function save_results {
	local TEST_NAME="$1"
	local result="$2"

	if [ -z "$result" ]; then
		die "no result output"
	fi

	metrics_json_init
	save_config

	metrics_json_start_array

	local result_line=$(echo "$result" | tail -1)
	local -a results
	read -a results <<< ${result_line%$'\r'}

	local total_bandwidth=${results[6]}
	local total_bandwidth_units=${results[7]}
	local total_loss=${results[16]}
	local total_loss_units="%"

	local json="$(cat << EOF
	{
		"bandwidth": {
			"Result" : $total_bandwidth,
			"Units"  : "$total_bandwidth_units"
		},
		"packet loss": {
			"Result" : $total_loss,
			"Units"  : "$total_loss_units"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save
}

function help {
echo "$(cat << EOF
Usage: $0 "[options]"
	Description:
		This script measures the UDP network bandwidth
		using different buffer sizes.

	Options:
		-a      Run all nuttcp tests
		-b      Run with default buffer size (8972)
		-c      Run with specific buffer size (1200)
		-h      Shows help
EOF
)"
}

function save_config {
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"image": "$image",
		"transmit timeout": "$transmit_timeout",
		"nuttcp version": "$nuttcp_version"
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"
}

function main() {
	local OPTIND
	while getopts ":abch:" opt
	do
		case "$opt" in
		a)      # Run all nuttcp tests
			test_bandwidth="1"
			;;
		b)      # UDP bandwidth with default buffer size
			test_bandwidth_default="1"
			;;
		c)      # UDP bandwidth with specific buffer size
			test_bandwidth_specific="1"
			;;
		h)
			help
			exit 0;
			;;
		\?)
			echo "An invalid option has been entered: -$OPTARG";
			help
			exit 1;
			;;
		:)
			echo "Missing argument for -$OPTARG";
			help
			exit 1;
			;;
		esac
	done
	shift $((OPTIND-1))

	[[ -z "$test_bandwidth" ]] && \
	[[ -z "$test_bandwidth_default" ]] && \
	[[ -z "$test_bandwidth_specific" ]] && \
		help && die "Must choose at least one test"

	# Check tools/commands dependencies
	cmds=("docker")
	check_cmds "${cmds[@]}"
	check_dockerfiles_images "$image" "$dockerfile"

	# Check no processes are left behind
	check_processes

	# Initialize/clean environment
	init_env

 	if [ "$test_bandwidth" == "1" ]; then
 		udp_default_buffer_size
  		udp_specific_buffer_size
	fi

	if [ "$test_bandwidth_default" == "1" ]; then
		udp_default_buffer_size
	fi

	if [ "$test_bandwidth_specific" == "1" ]; then
		udp_specific_buffer_size
	fi
}

main "$@"

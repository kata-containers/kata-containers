#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This test measures the following network essentials:
# - bandwith simplex
# - bandwith duplex
# - jitter
#
# These metrics/results will be got from the interconnection between
# a client and a server using iperf3 tool.
# The following cases are covered:
#
# case 1:
#  container-server <----> container-client
#
# case 2"
#  container-server <----> host-client

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/lib/network-common.bash"
source "${SCRIPT_PATH}/../lib/common.bash"

# Port number where the server will run
port="${port:-5201}"
fwd_port="${fwd_port:-$port:$port}"
# Image name
image="${image:-local-iperf3}"
# Dockerfile
dockerfile="${SCRIPT_PATH}/iperf3_dockerfile/Dockerfile"
# Measurement time (seconds)
transmit_timeout="${transmit_timeout:-30}"

# Iperf server configuration
# Set the TMPDIR to an existing tmpfs mount to avoid a 9p unlink error
init_cmds="export TMPDIR=/dev/shm"
server_command="$init_cmds && iperf3 -s"

# Test single direction TCP bandwith
function iperf3_bandwidth() {
	local TEST_NAME="network iperf3 bandwidth"
	cmds=("bc" "jq")
	check_cmds "${cmds[@]}"

	# Check no processes are left behind
	check_processes

	init_env

	local server_address=$(start_server "$image" "$server_command" "$server_extra_args")

	# Verify server IP address
	if [ -z "$server_address" ];then
		clean_env
		die "server: ip address no found"
	fi

	# Start client
	local client_command="$init_cmds && iperf3 -J -c ${server_address} -t ${transmit_timeout}"

	metrics_json_init
	save_config

	# Start server
	result=$(start_client "$image" "$client_command" "$client_extra_args")

	metrics_json_start_array

	local bits_per_second=$(echo "$result" | jq '.end.sum_received.bits_per_second')
	local total_bandwidth=$(echo "scale=2 ; $bits_per_second / 1000000" | bc)

	local json="$(cat << EOF
 	{
 		"bandwidth": {
			"Result" : $total_bandwidth,
			"Units"  : "Mbps"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"

	# Saving raw results
	metrics_json_start_array
	metrics_json_add_array_element "$result"
	metrics_json_end_array "Raw Results"

	metrics_json_save
	clean_env

	# Check no processes are left behind
	check_processes
}

# Test jitter on single direction UDP
function iperf3_jitter() {
	local TEST_NAME="network iperf3 jitter"
	cmds=("jq")
	check_cmds "${cmds[@]}"

	# Check no processes are left behind
	check_processes
	init_env

	# Start server
	local server_address=$(start_server "$image" "$server_command" "$server_extra_args")

	# Verify server IP address
	if [ -z "$server_address" ];then
		clean_env
		die "server: ip address no found"
	fi

	metrics_json_init
	save_config

	# Start server
	local client_command="$init_cmds && iperf3 -J -c ${server_address} -u -t ${transmit_timeout}"
	result=$(start_client "$image" "$client_command" "$client_extra_args")

	metrics_json_start_array

	local total_jitter=$(echo "$result" | jq '.end.sum.jitter_ms')

	local json="$(cat << EOF
	{
		"jitter": {
			"Result" : $total_jitter,
			"Units"  : "ms"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"

	# Saving raw results
	metrics_json_start_array
	metrics_json_add_array_element "$result"
	metrics_json_end_array "Raw Results"

	metrics_json_save
	clean_env

	# Check no processes are left behind
	check_processes
}

# This function checks/verify if the iperf3 server
# is ready/up for requests.
function check_iperf3_server() {
	local retry=6
	local period=0.5
	local test_cmd="iperf3 -c "$server_address" -t 1"

	# check tools dependencies
	local cmds=("netstat" "iperf3")
	check_cmds "${cmds[@]}"

	while [ 1 ]; do
		if ! bash -c "$test_cmd" > /dev/null 2>&1; then
			echo "waiting for server..."
			(( count++ ))
			sleep $period
		else
			echo "iperf3 server is up!"
			break;
		fi

		if [ "$count" -ge "$retry" ]; then
			die "iperf3 server init fails"
		fi
	done

	# Check listening port
	lport="$(netstat -atun | grep "$port" | grep "LISTEN")"
	if [ -z "$lport" ]; then
		die "port is not listening"
	fi
}

# This function parses the output of iperf3 execution
function parse_iperf3_bwd() {
	local TEST_NAME="$1"
	local result="$2"

	if [ -z "$result" ]; then
		die "no result output"
	fi

	metrics_json_init
	save_config
	metrics_json_start_array

 	# Getting results
	local rx_bwd=$(echo "$result" | jq '.end.sum_received.bits_per_second')
	local rx_bwd_mbps=$(echo "scale=2 ; $rx_bwd / 1000000" | bc)
	local tx_bwd=$(echo "$result" | jq '.end.sum_sent.bits_per_second')
	local tx_bwd_mbps=$(echo "scale=2 ; $tx_bwd / 1000000" | bc)

	local json="$(cat << EOF
	{
 		"receiver": {
			"Result" : $rx_bwd_mbps,
			"Units"  : "Mbps"
		},
		"sender": {
			"Result" : $tx_bwd_mbps,
			"Units"  : "Mbps"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"

	# Saving raw results
	metrics_json_start_array
	metrics_json_add_array_element "$result"
	metrics_json_end_array "Raw Results"

	metrics_json_save
}

# This function parses the output of iperf3 UDP execution, and
# saves the receiver successful datagram value in the results.
function parse_iperf3_pps() {
	local TEST_NAME="$1"
	local result="$2"

	if [ -z "$result" ]; then
		die "no result output"
	fi

	metrics_json_init
	save_config

	metrics_json_start_array

	# Extract results
	local lost=$(echo "$result" | jq '.end.sum.lost_packets')
 	local total=$(echo "$result" | jq '.end.sum.packets')
	local notlost=$((total-lost))
	local pps=$((notlost/transmit_timeout))

	local json="$(cat << EOF
	{
		"receiver": {
			"Result" : $pps,
			"Units"  : "PPS"
		},
		"lost":     {
			"Result" : $lost,
			"Units"  : "PPS"
		},
		"total":    {
			"Result" : $total,
			"Units"  : "PPS"
		},
		"not lost":  {
			"Result" : $notlost,
			"Units"  : "PPS"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"

	# Saving raw results
	metrics_json_start_array
	metrics_json_add_array_element "$result"
	metrics_json_end_array "Raw Results"

	metrics_json_save
}

# This function launches a container that will take the role of
# server, this is order to attend requests from a client.
# In this case the client is an instance of iperf3 running in the host.
function get_host_cnt_bwd() {
	local cli_args="$1"

	# Check no processes are left behind
	check_processes 1>&2

	# Initialize/clean environment
	init_env 1>&2

	# Make port forwarding
	local server_extra_args="$server_extra_args -p $fwd_port"
	local server_address=$(start_server "$image" "$server_command" "$server_extra_args")

	# Verify server IP address
	if [ -z "$server_address" ];then
 		clean_env
		die "server: ip address no found"
	fi

	# Verify the iperf3 server is up
	check_iperf3_server 1>&2

	# client test executed in host
	local output=$(iperf3 -J -c $server_address -t $transmit_timeout "$cli_args")

	clean_env 1>&2
	echo "$output"

	# Check no processes are left behind
	check_processes 1>&2
}

# Run a UDP PPS test between two containers.
# Use the smallest packets we can and run with unlimited bandwidth
# mode to try and get as many packets through as possible.
function get_cnt_cnt_pps() {
	local cli_args="$1"

	# Check we have the json query tool to parse the results
	local cmds=("jq" "iperf3")
	check_cmds "${cmds[@]}" 1>&2

	# Check no processes are left behind
	check_processes 1>&2

	# Initialize/clean environment
	#  We need to do the stdout re-direct as we don't want any verbage in the
	#  answer we return, as then it is not a valid JSON result...
	init_env 1>&2

	# Make port forwarding
	local server_extra_args="$server_extra_args -p $fwd_port"
	local server_address=$(start_server "$image" "$server_command" "$server_extra_args")

	# Verify server IP address
	if [ -z "$server_address" ];then
		clean_env
		die "server: ip address no found"
	fi

	# Verify the iperf3 server is up
	check_iperf3_server 1>&2

	# and start the client container
	local client_command="$init_cmds && iperf3 -J -u -c ${server_address} -l 64 -b 0 ${cli_args} -t ${transmit_timeout}"
	local output=$(start_client "$image" "$client_command" "$client_extra_args")

	clean_env 1>&2
	echo "$output"

	# Check no processes are left behind
	check_processes 1>&2
}

# Run a UDP PPS test between the host and a container, with the client on the host.
# Use the smallest packets we can and run with unlimited bandwidth
# mode to try and get as many packets through as possible.
function get_host_cnt_pps() {
	local cli_args="$1"

	# Checks iperf3 tool installed in host
	# We also need the json query tool, as we use JSON format results from iperf3
	local cmds=("iperf3" "jq")
	check_cmds "${cmds[@]}" 1>&2

	# Check no processes are left behind
	check_processes 1>&2

	# Initialize/clean environment
	init_env 1>&2

	# Make port forwarding
	local server_extra_args="$server_extra_args -p $fwd_port"
	local server_address=$(start_server "$image" "$server_command" "$server_extra_args")

	# Verify server IP address
	if [ -z "$server_address" ];then
		clean_env
		die "server: ip address no found"
	fi

	# Verify the iperf3 server is up
	check_iperf3_server 1>&2

	# and start the client container
	local output=$(iperf3 -J -u -c $server_address -l 64 -b 0 -t $transmit_timeout "$cli_args")

	clean_env 1>&2
	echo "$output"

	# Check no processes are left behind
	check_processes 1>&2
}

# This test measures the bandwidth between a container and the host.
# where the container take the server role and the iperf3 client lives
# in the host.
function iperf3_host_cnt_bwd() {
	local TEST_NAME="network bwd host contr"
	local result="$(get_host_cnt_bwd)"
	parse_iperf3_bwd "$TEST_NAME" "$result"
}

# This test is similar to "iperf3_host_cnt_bwd", the difference is this
# tests runs in reverse mode.
function iperf3_host_cnt_bwd_rev() {
	local TEST_NAME="network bwd host contr reverse"
	local result="$(get_host_cnt_bwd "-R")"
	parse_iperf3_bwd "$TEST_NAME" "$result"
}

# This tests measures the bandwidth using different number of parallel
# client streams. (2, 4, 8)
function iperf3_multiqueue() {
	local TEST_NAME="network multiqueue"
	local client_streams=("2" "4" "8")

	for s in "${client_streams[@]}"; do
		tn="$TEST_NAME $s"
		result="$(get_host_cnt_bwd "-P $s")"
		parse_iperf3_bwd "$tn" "$result"
	done
}

# This test measures the packet-per-second (PPS) between two containers.
# It uses the smallest (64byte) UDP packet streamed with unlimited bandwidth
# to obtain the result.
function iperf3_cnt_cnt_pps() {
	local TEST_NAME="network pps cnt cnt"
	local result="$(get_cnt_cnt_pps)"
	parse_iperf3_pps "$TEST_NAME" "$result"
}

# This test measures the packet-per-second (PPS) between the host and a container.
# It uses the smallest (64byte) UDP packet streamed with unlimited bandwidth
# to obtain the result.
function iperf3_host_cnt_pps() {
	local TEST_NAME="network pps host cnt"
	local result="$(get_host_cnt_pps)"
	parse_iperf3_pps "$TEST_NAME" "$result"
}

# This test measures the packet-per-second (PPS) between the host and a container.
# It runs iperf3 in 'Reverse' mode.
# It uses the smallest (64byte) UDP packet streamed with unlimited bandwidth
# to obtain the result.
function iperf3_host_cnt_pps_rev() {
	local TEST_NAME="network pps host cnt rev"
	local result="$(get_host_cnt_pps "-R")"
	parse_iperf3_pps "$TEST_NAME" "$result"
}

save_config(){
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"image": "$image",
		"transmit timeout": "$transmit_timeout",
		"iperf3 version": "3.3"
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"
}


function help {
echo "$(cat << EOF
Usage: $0 "[options]"
	Description:
		This script implements a number of network metrics
		using iperf3.

	Options:
		-a      Run all tests
		-b      Run all bandwidth tests
		-h      Shows help
		-j      Run jitter tests
		-p      Run all PPS tests
EOF
)"
}

function main {
	local OPTIND
 	while getopts ":abh:jpt:" opt
	do
		case "$opt" in
		a)      # all tests
			test_bandwidth="1"
			test_jitter="1"
			test_pps="1"
			;;
		b)      # bandwidth tests
			test_bandwidth="1"
			;;
		h)
			help
			exit 0;
			;;
		j)      # Jitter tests
			test_jitter="1"
			;;
		p)      # all PacketPerSecond tests
			test_pps="1"
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
	[[ -z "$test_jitter" ]] && \
	[[ -z "$test_pps" ]] && \
		help && die "Must choose at least one test"

	check_dockerfiles_images "$image" "$dockerfile"

	if [ "$test_bandwidth" == "1" ]; then
 		iperf3_bandwidth
		iperf3_host_cnt_bwd
 		iperf3_host_cnt_bwd_rev
		iperf3_multiqueue
	fi

	if [ "$test_jitter" == "1" ]; then
		iperf3_jitter
	fi

	if [ "$test_pps" == "1" ]; then
		iperf3_cnt_cnt_pps
		iperf3_host_cnt_pps
		iperf3_host_cnt_pps_rev
	fi
}

main "$@"

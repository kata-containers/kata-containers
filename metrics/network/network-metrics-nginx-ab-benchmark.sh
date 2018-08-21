#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description:
# This test measures the request per second from the interconnection
# between a server container and the host using ab tool.
# container-server <----> host

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

# Test name
TEST_NAME="${TEST_NAME:-network nginx ab benchmark}"
# Ports where it will run
port="${port:-80:80}"
# Image name
image="${image:-nginx}"
# Url
url="${url:-localhost:80}"
# Number of requests to perform
# (large number to reduce standard deviation)
requests="${requests:-10000}"
# Start time (let the container to start correctly)
start_time="${start_time:-2}"
# File to save ab results
TMP_FILE=$(mktemp results.XXXXXXXXXX)
# Concurrency
concurrency="${concurrency:-100}"

function remove_tmp_file {
	rm -rf $TMP_FILE
}

trap remove_tmp_file EXIT

function save_config {
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"image": "$image",
		"requests": "$requests",
		"concurrency" : "$concurrency"
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"
}

function main {
	# Check tools/commands dependencies
	cmds=("ab" "awk")
	check_cmds "${cmds[@]}"

	# Initialize/clean environment
	init_env
	check_images "$image"

	# Launch nginx container
	docker run -d -p $port $image
	echo >&2 "WARNING: sleeping for $start_time seconds to let the container start correctly"
	sleep "$start_time"
	ab -n ${requests} -c ${concurrency} http://${url}/ > $TMP_FILE

	metrics_json_init
	save_config

	metrics_json_start_array
	local total_time=$(cat $TMP_FILE | awk '/Time taken for tests/ {print $5}')
	local total_complete_requests=$(cat $TMP_FILE | awk '/Complete requests/ {print $3}')
	local total_failed_requests=$(cat $TMP_FILE | awk '/Failed requests/ {print $3}')
	local total_transferred=$(cat $TMP_FILE | awk '/Total transferred/ {print $3}')
	local total_requests_per_second=$(cat $TMP_FILE | awk '/Requests per second/ {print $4}')
	local total_transfer_rate=$(cat $TMP_FILE | awk '/Transfer rate/ {print $3}')

	local json="$(cat << EOF
	{
		"General output": {
			"Time taken for tests (seconds)"  : $total_time,
			"Complete requests" : $total_complete_requests,
			"Failed requests" : $total_failed_requests,
			"Total transferred (bytes)" : $total_transferred,
			"Requests per second" : $total_requests_per_second,
			"Transfer rate (Kbyte/sec)": $total_transfer_rate
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save

	clean_env
}

main "$@"

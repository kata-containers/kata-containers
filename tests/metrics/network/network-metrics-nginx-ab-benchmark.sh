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

# Ports where it will run
port="${port:-80:80}"
# Url
url="${url:-localhost:80}"
# Number of requests to perform
# (large number to reduce standard deviation)
requests="${requests:-10000}"
# Start time (let the container to start correctly)
start_time="${start_time:-2}"
# File to save ab results
TMP_FILE=$(mktemp results.XXXXXXXXXX)
# Maximum number of seconds to wait before
# the  socket  times  out
socket_time="${socket_time:-120}"

function remove_tmp_file {
	rm -rf $TMP_FILE
}

trap remove_tmp_file EXIT

function concurrency {
	local concurrency_value=$1
	local TEST_NAME="${TEST_NAME:-network nginx ab benchmark with ${concurrency_value} concurrency}"
	local result="$(nginx_ab_networking)"
	save_results "$TEST_NAME"
}

function save_config {
	metrics_json_start_array

	local json="$(cat << EOF
	{
		"image": "$image",
		"requests": "$requests",
		"concurrency" : "$concurrency_value"
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
		This test measures the request per second from the interconnection
		between a server container and the host using ab tool.
	Options:
		-n      Run with other concurrency value (enter the value)
		-h      Shows help
EOF
)"
}

function nginx_ab_networking {
	# Launch nginx container
	docker run --runtime "$RUNTIME" -d -p $port $nginx_image
	echo >&2 "WARNING: sleeping for $start_time seconds to let the container start correctly"
	sleep "$start_time"
	ab -s ${socket_time} -n ${requests} -r -c ${concurrency_value} http://${url}/ > $TMP_FILE

	clean_env

	# Check no processes are left behind
	check_processes
}

function save_results {
	local TEST_NAME="$1"
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
}

function main() {
	local OPTIND
	while getopts ":h:n:" opt
	do
		case "$opt" in
		n)	# Run with specific concurrency value
			concurrency_value="$OPTARG"
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

	[[ -z "$concurrency_value" ]] && \
		help && die "Must choose at least one test"

	# Check tools/commands dependencies
	cmds=("ab" "awk")
	check_cmds "${cmds[@]}"

	# Check no processes are left behind
	check_processes

	# Initialize/clean environment
	init_env
	versions_file="${SCRIPT_PATH}/../../versions.yaml"
	nginx_version=$("${GOPATH}/bin/yq" read "$versions_file" "docker_images.nginx.version")
	nginx_image="nginx:$nginx_version"
	check_images "$nginx_image"

	concurrency $concurrency_value
}
main "$@"

#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Description of the test:
# This test runs the 'iperf' network benchmark and
# we retrieve cpu information while running the networking
# test with 'perf'. CPU utilization provides more stable
# results that help to detect performace regressions compared
# with the actual throughput.

set -e

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

# Test name
TEST_NAME="${TEST_NAME:-cpu information}"
# Image name
IMAGE="${IMAGE:-local-iperf}"
# Dockerfile
DOCKERFILE="${SCRIPT_PATH}/iperf_dockerfile/Dockerfile"
# Client name
CLIENT_NAME="${CLIENT_NAME:-client_iperf}"
# Server name
SERVER_NAME="${SERVER_NAME:-server_iperf}"
# Target bandwidth
BANDWIDTH="${BANDWIDTH:-1G}"
# Time in seconds to transmit
TIMEOUT="${TIMEOUT:-160}"
# Sample period in seconds to measure
SAMPLE_PERIOD="${SAMPLE_PERIOD:-120}"
# File to save perf results
TMP_FILE=$(mktemp cpuinfo.XXXXXXXXXX)
# Time in seconds is required to let the benchmark settle down
SETTLE_TIME="${SETTLE_TIME:-20}"

function remove_tmp_file() {
	rm -rf $TMP_FILE
}

trap remove_tmp_file EXIT

function main() {
	# Check tools/commands dependencies
	cmds=("awk" "docker" "perf")

	# Check no processes are left behind
	check_processes

	init_env
	check_cmds "${cmds[@]}"
	check_dockerfiles_images "$IMAGE" "$DOCKERFILE"

	# Start iperf server configuration
	# Set the TMPDIR to an existing tmpfs mount to avoid a 9p unlink error
	local init_cmds="export TMPDIR=/dev/shm"
	local server_cmd="$init_cmds && iperf -s"
	docker run --runtime $RUNTIME --name $SERVER_NAME -d $IMAGE bash -c "$server_cmd"
	local server_address=$(docker inspect --format "{{.NetworkSettings.IPAddress}}" $SERVER_NAME)

	# Start iperf client
	local client_cmd="$init_cmds && iperf -c $server_address -b $BANDWIDTH -t $TIMEOUT"
	docker run --runtime $RUNTIME --name $CLIENT_NAME -d $IMAGE bash -c "$client_cmd"

 	# This time in seconds is required to let the benchmark settle down
 	sleep "$SETTLE_TIME"

 	# Retrieve qemu information
 	PIDS="$(pgrep -d ',' qemu)"

 	metrics_json_init

 	# Start collecting cpu information
 	sudo perf stat -a -o $TMP_FILE -e cycles -e instructions -p ${PIDS} sleep ${SAMPLE_PERIOD}

 	# Save configuration
 	metrics_json_start_array

	local instructions="$(cat $TMP_FILE | grep -w instructions | awk '{print $1}' | tr ',' ' ' | tr -d ' ')"

	local cycles="$(cat $TMP_FILE | grep -w cycles | awk '{print $1}' | tr ',' ' ' | tr -d ' ')"

	local instructions_per_cycle="$(cat $TMP_FILE | grep -w 'per cycle' | cut -d '#' -f2 | awk '{print $1}')"

	local units_instructions_per_cycle="$(cat $TMP_FILE |  grep -w 'per cycle' | cut -d '#' -f2 |  awk '{print $2}')"

	local json="$(cat << EOF
	{
		"instructions per cycle": {
			"Result" : $instructions_per_cycle,
			"Units"  : "$units_instructions_per_cycle per cycle"
		},
		"cycles": {
			"Result" : $cycles,
			"Units"  : "cycles"
		},
		"instructions": {
			"Result" : $instructions,
			"Units"  : "instructions"
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

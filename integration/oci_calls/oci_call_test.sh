#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will verify the arguments of
# running or stopping a container matches
# with the OCI calls

set -e

dir_path=$(dirname "$0")
source "${dir_path}/../../lib/common.bash"
source /etc/os-release || source /usr/lib/os-release

# Save logs
TMP_FILE=$(mktemp runtimelogs.XXXXX)
# Environment variables
IMAGE="busybox"
PAYLOAD="tail -f /dev/null"
NAME="testoci"

function remove_tmp_file {
	rm "$TMP_FILE"
}

trap remove_tmp_file EXIT

# Get time to filter the logs
function get_time() {
	start_time=$(date "+%F %H:%M:%S")
}

# Get log for a specific time
function get_debug_logs() {
	sudo journalctl -q --since "$start_time" -o cat -a -t "${RUNTIME}" > "${TMP_FILE}"
}

# Find the arguments or oci calls for a specific command
function check_arguments() {
	list_arguments=$(grep -o "arguments=[^ ]*" "${TMP_FILE}" --color|cut -d= -f2-|tr -d '"'|tr -d "\\\\")

	[ -n "$list_arguments" ] || die "List of arguments missing"

	# Check arguments vs oci calls
	for i in "${oci_call[@]}"; do
		echo "$list_arguments" | grep -w "$i" > /dev/null
	done
}

# Find the order of the arguments is equal to the order of the oci call
function order_arguments() {
	# Remove all duplicated arguments, remove `state` argument (as it is
	# not defined with a specific order and we already checked that is part
	# of the oci arguments) and remove an empty space.
	local -a final_arguments
	final_arguments=$(echo "${list_arguments//state/}" | \
		awk '{for (i=1;i<=NF;i++) if (!a[$i]++) printf("%s%s",$i,FS)}' | \
		sed 's/ *$//')
	final_oci="$(echo ${oci_call[*]//state/})"

	[[ "${final_oci}" == "${final_arguments}" ]]
}

function setup() {
	clean_env

	check_processes

	extract_kata_env

	# Enable full debug
	sudo sed -i 's/#enable_debug = true/enable_debug = true/g' "${RUNTIME_CONFIG_PATH}"
}

function run_oci_call() {
	local -a oci_call=( "create" "start" "state" )

	# This sleep is necessary to gather the correct logs
	sleep 10

	get_time

	# Start a container
	docker run -d --runtime="${RUNTIME}" --name="${NAME}" "${IMAGE}" sh -c "${PAYLOAD}"

	get_debug_logs

	check_arguments

	order_arguments
}

function stop_oci_call() {
	local -a oci_call=( "kill" "delete" "state" )

	# This sleep is necessary to gather the correct logs
	sleep 10

	get_time

	# Stop a container
	docker stop ${NAME}

	get_debug_logs

	docker rm -f ${NAME}

	check_arguments

	order_arguments
}

function run_oci_call_true() {
	# Find docker version
	version=$(docker version --format '{{.Server.Version}}' | cut -d '.' -f1-2)
	result=$(echo "$version>18.06" | bc)
	if [ "${result}" -eq 1 ]; then
		local -a oci_call=( "create" "start" "delete" "state" )
	else
		local -a oci_call=( "create" "start" "kill" "delete" "state" )
	fi

	# This sleep is necessary to gather the correct logs
	sleep 10

	get_time

	# Run a container with true
	docker run --rm --runtime="${RUNTIME}" "${IMAGE}" true

	get_debug_logs

	check_arguments

	order_arguments
}

function teardown() {
	clean_env

	check_processes

	extract_kata_env

	# Disable full debug
	sudo sed -i 's/enable_debug = true/#enable_debug = true/g' "${RUNTIME_CONFIG_PATH}"
}

echo "Running setup"
setup

echo "Check oci calls for run"
run_oci_call

echo "Check oci calls for stop"
stop_oci_call

echo "Check oci calls for run with true"
run_oci_call_true

echo "Teardown"
teardown

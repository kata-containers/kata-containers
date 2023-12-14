#!/bin/bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o pipefail

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")

source "${SCRIPT_PATH}/../../lib/common.bash"
latency_file=$(mktemp latencyresults.XXXXXXXXXX)
TEST_NAME="${TEST_NAME:-latency}"

function remove_tmp_file() {
	rm -rf "${latency_file}"
}

trap remove_tmp_file EXIT

function latency_cleanup() {
	info "Latency test cleanup"
	kubectl delete -f "${SCRIPT_PATH}/latency-server.yaml"
	kubectl delete -f "${SCRIPT_PATH}/latency-client.yaml"
}

function main() {
	init_env
	cmds=("bc" "jq")
	check_cmds "${cmds[@]}"

	init_env

	# Check no processes are left behind
	check_processes

	wait_time=20
	sleep_time=2

	# Create server
	kubectl create -f "${SCRIPT_PATH}/latency-server.yaml"

	# Get the names of the server pod
	export server_pod_name="latency-server"

	# Verify the server pod is working
	local cmd="kubectl get pod $server_pod_name -o yaml | grep 'phase: Running'"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Create client
	kubectl create -f "${SCRIPT_PATH}/latency-client.yaml"

	trap latency_cleanup EXIT

	# Get the names of the client pod
	export client_pod_name="latency-client"

	# Verify the client pod is working
	local cmd="kubectl get pod $client_pod_name -o yaml | grep 'phase: Running'"
	waitForProcess "$wait_time" "$sleep_time" "$cmd"

	# Get the ip address of the server pod
	export server_ip_add=$(kubectl get pod "$server_pod_name" -o jsonpath='{.status.podIP}')

	# Number of packets (sent)
	local number="${number:-30}"

	local client_command="ping -c ${number} ${server_ip_add}"

	kubectl exec "$client_pod_name" -- sh -c "$client_command" > "$latency_file"

	metrics_json_init

	local latency=$(cat $latency_file | grep avg | cut -f2 -d'=' | sed 's/[[:blank:]]//g' | cut -f2 -d'/')

	metrics_json_start_array

	local json="$(cat << EOF
	{
		"latency": {
			"Result" : $latency,
			"Units" : "ms"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save
}
main "$@"

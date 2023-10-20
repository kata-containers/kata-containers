#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This test measures the following UDP network essentials:
# - bandwith simplex
# - parallel bandwidth
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

set -o pipefail

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")

source "${SCRIPT_PATH}/../../lib/common.bash"
iperf_file=$(mktemp iperfresults.XXXXXXXXXX)
TEST_NAME="${TEST_NAME:-network-iperf3-udp}"
COLLECT_ALL="${COLLECT_ALL:-false}"
IPERF_DEPLOYMENT="${SCRIPT_PATH}/runtimeclass_workloads/iperf3-deployment.yaml"
IPERF_DAEMONSET="${SCRIPT_PATH}/runtimeclass_workloads/iperf3-daemonset.yaml"

function remove_tmp_file() {
	rm -rf "${iperf_file}"
}

trap remove_tmp_file EXIT

function iperf3_udp_all_collect_results() {
	metrics_json_init
	metrics_json_start_array
	local json="$(cat << EOF
	{
		"bandwidth": {
			"Result" : $bandwidth_result,
			"Units" : "$bandwidth_units"
		},
		"parallel": {
			"Result" : $parallel_result,
			"Units" : "$parallel_units"
		}
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
}

function iperf3_udp_bandwidth() {
	# Start server
	local transmit_timeout="120"

	kubectl exec -i "$client_pod_name" -- sh -c "iperf3 -c ${server_ip_add} -u -b 1G -t $transmit_timeout" | grep receiver | cut -d' ' -f13 > "${iperf_file}"
	export bandwidth_result=$(cat "${iperf_file}")
	export bandwidth_units="Mbits/sec"

	if [ "$COLLECT_ALL" == "true" ]; then
		iperf3_udp_all_collect_results
	else
		metrics_json_init
		metrics_json_start_array

		local json="$(cat << EOF
		{
			"bandwidth": {
				"Result" : $bandwidth_result,
				"Units" : "$bandwidth_units"
			}
		}
EOF
)"
		metrics_json_add_array_element "$json"
		metrics_json_end_array "Results"
	fi
}

function iperf3_udp_parallel() {
	# Start server
	local transmit_timeout="120"

	kubectl exec -i "$client_pod_name" -- sh -c "iperf3 -c ${server_ip_add} -u -J -P 4" | jq '.end.sum.bits_per_second' > "${iperf_file}"
	export parallel_result=$(cat "${iperf_file}")
	export parallel_units="bits/sec"

	if [ "$COLLECT_ALL" == "true" ]; then
		iperf3_udp_all_collect_results
	else
		metrics_json_init
		metrics_json_start_array

		local json="$(cat << EOF
		{
			"parallel": {
				"Result" : $parallel_result,
				"Units" : "$parallel_units"
			}
		}
EOF
)"
		metrics_json_add_array_element "$json"
		metrics_json_end_array "Results"
	fi
}

function iperf3_udp_start_deployment() {
	cmds=("bc")
	check_cmds "${cmds[@]}"

	# Check no processes are left behind
	check_processes

	wait_time=20
	sleep_time=2

	# Create deployment
	kubectl create -f "${IPERF_DEPLOYMENT}"

	# Check deployment creation
	local cmd="kubectl wait --for=condition=Available deployment/iperf3-server-deployment"
	waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

	# Create DaemonSet
	kubectl create -f "${IPERF_DAEMONSET}"

	# Get the names of the server pod
	export server_pod_name=$(kubectl get pods -o name | grep server | cut -d '/' -f2)

	# Verify the server pod is working
	local cmd="kubectl get pod ${server_pod_name} -o yaml | grep 'phase: Running'"
	waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

	# Get the names of client pod
	export client_pod_name=$(kubectl get pods -o name | grep client | cut -d '/' -f2)

	# Verify the client pod is working
	local cmd="kubectl get pod ${client_pod_name} -o yaml | grep 'phase: Running'"
	waitForProcess "${wait_time}" "${sleep_time}" "${cmd}"

	# Get the ip address of the server pod
	export server_ip_add=$(kubectl get pod "${server_pod_name}" -o jsonpath='{.status.podIP}')
}

function iperf3_udp_deployment_cleanup() {
	info "iperf: deleting deployments and services"
	kubectl delete pod "${server_pod_name}" "${client_pod_name}"
	kubectl delete -f "${IPERF_DAEMONSET}"
	kubectl delete -f "${IPERF_DEPLOYMENT}"
	kill_kata_components && sleep 1
	kill_kata_components
	check_processes
	info "End of iperf3 test"
}

# The deployment must be removed in
# any case the script terminates.
trap iperf3_udp_deployment_cleanup EXIT

function help() {
echo "$(cat << EOF
Usage: $0 "[options]"
	Description:
		This script implements a number of network metrics
		using iperf3 with UDP.

	Options:
		-a      Run all tests
		-b      Run bandwidth tests
		-p	Run parallel tests
		-h      Help
EOF
)"
}

function main() {
	init_env
	iperf3_udp_start_deployment

	local OPTIND
	while getopts ":abph:" opt
	do
		case "$opt" in
		a)      # all tests
			test_all="1"
			;;
		b)      # bandwith test
			test_bandwith="1"
			;;
		p)	# parallel test
			test_parallel="1"
			;;
		h)
			help
			exit 0;
			;;
		:)
			echo "Missing argument for -$OPTARG";
			help
			exit 1;
			;;
		esac
	done
	shift $((OPTIND-1))

	[[ -z "$test_bandwith" ]] && \
	[[ -z "$test_parallel" ]] && \
	[[ -z "$test_all" ]] && \
		help && die "Must choose at least one test"

	if [ "$test_bandwith" == "1" ]; then
		iperf3_udp_bandwidth
	fi

	if [ "$test_parallel" == "1" ]; then
		iperf3_udp_parallel
	fi

	if [ "$test_all" == "1" ]; then
		export COLLECT_ALL=true && iperf3_udp_bandwidth && iperf3_udp_parallel
	fi

	info "iperf3: saving test results"
	metrics_json_save
}

main "$@"
